//! bone_trace — le relevé d'os du personnage, par SQUELETTE et par NOM.
//!
//! # Ce que la version d'avant ne pouvait pas voir
//!
//! Elle itérait les **maillages skinnés**, pas les squelettes. Le chien
//! d'expédition en a huit (museau, gueule, blanc d'œil, iris, pupille, sac,
//! corps, tissu) : ils comptaient pour **huit personnages**, saturaient le
//! plafond `max_characters = 8`, et chacun relevait les `max_bones = 24`
//! premiers os — c'est-à-dire **huit fois les mêmes vingt-quatre**.
//!
//! `RightArm`, `RightForeArm` et `RightHand` n'ont donc **jamais** été relevés.
//! Le 2026-08-16, une session entière a diagnostiqué une pose de bras à l'œil
//! sur une capture d'écran pendant que ce fichier s'écrivait toutes les deux
//! secondes, à côté, sans les bras dedans.
//!
//! Un plafond qui coupe toujours au même endroit ne mesure pas « un
//! échantillon » : il mesure **toujours la même chose**, et rend le reste
//! structurellement invisible. D'où les deux changements :
//!
//! 1. **Un squelette = une entrée.** Les maillages qui le partagent sont
//!    listés dedans, pas à côté.
//! 2. **Les os SURVEILLÉS d'abord**, désignés par leur nom au génome. Le
//!    plafond ne remplit que ce qui reste, et le nombre d'os écartés est
//!    publié — un relevé tronqué qui ne dit pas qu'il tronque se lit comme un
//!    relevé complet.

use bevy::camera::primitives::Aabb;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use forgia_core::prelude::*;

/// Fichier de réglages. Il EXISTE depuis story-454 et n'était lu par personne :
/// son en-tête se disait « hot-reloadable », aucun code ne l'ouvrait, et les
/// valeurs qui tournaient étaient celles écrites en dur ici. Il est lu depuis le
/// 2026-08-16.
const GENOME: &str = "config/genomes/debug_anim.toml";

/// Réglages du relevé. Lus au démarrage depuis [`GENOME`].
#[derive(Resource, Debug, Clone)]
pub struct BoneTraceConfig {
    pub enabled: bool,
    pub hz: f32,
    /// Plafond d'os relevés PAR SQUELETTE, après les surveillés.
    pub max_bones: usize,
    /// Plafond de squelettes distincts — plus de maillages, de vrais squelettes.
    pub max_squelettes: usize,
    pub desync_threshold_m: f32,
    /// Les os qu'on veut voir QUOI QU'IL ARRIVE, par nom. C'est la liste qui
    /// rend le relevé utile : sans elle, on relève l'ordre du fichier glTF, qui
    /// ne dit rien de ce qu'on cherche.
    pub os_surveilles: Vec<String>,
}

impl Default for BoneTraceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            hz: 2.0,
            max_bones: 24,
            max_squelettes: 4,
            desync_threshold_m: 0.05,
            // Les os d'une chaîne de tir et d'appui : ce qui porte l'arme, ce
            // qui vise, ce qui touche le sol. Ce sont ceux dont un défaut se
            // voit à l'écran.
            os_surveilles: [
                "Hips",
                "Spine2",
                "Neck",
                "RightShoulder",
                "RightArm",
                "RightForeArm",
                "RightHand",
                "LeftArm",
                "LeftForeArm",
                "LeftHand",
                "RightFoot",
                "LeftFoot",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        }
    }
}

impl BoneTraceConfig {
    /// Lit le génome, ou retombe sur les défauts **en le disant**.
    #[must_use]
    pub fn charger() -> Self {
        let mut cfg = Self::default();
        let Ok(src) = forgia_core::def_io::read_def_str(GENOME) else {
            warn!("[bone-trace] {GENOME} illisible — réglages par défaut");
            return cfg;
        };
        for ligne in src.lines() {
            let ligne = ligne.trim();
            let Some((cle, val)) = ligne.split_once('=') else {
                continue;
            };
            let (cle, val) = (cle.trim(), val.split('#').next().unwrap_or("").trim());
            match cle {
                "dbg_bone_trace_enabled" => cfg.enabled = val != "0" && val != "false",
                "dbg_bone_trace_hz" => cfg.hz = val.parse().unwrap_or(cfg.hz),
                "dbg_bone_trace_max_bones" => {
                    cfg.max_bones = val.parse().unwrap_or(cfg.max_bones);
                }
                "dbg_bone_trace_max_squelettes" => {
                    cfg.max_squelettes = val.parse().unwrap_or(cfg.max_squelettes);
                }
                "dbg_bone_trace_desync_threshold_m" => {
                    cfg.desync_threshold_m = val.parse().unwrap_or(cfg.desync_threshold_m);
                }
                "dbg_bone_trace_os_surveilles" => {
                    let liste: Vec<String> = val
                        .trim_start_matches('[')
                        .trim_end_matches(']')
                        .split(',')
                        .map(|s| s.trim().trim_matches('"').to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    // Une liste vide au fichier ne doit PAS effacer la liste
                    // par défaut : on perdrait les os surveillés sans un mot.
                    if !liste.is_empty() {
                        cfg.os_surveilles = liste;
                    }
                }
                _ => {}
            }
        }
        info!(
            "[bone-trace] {GENOME} lu — {} os surveillés, plafond {} os / {} squelettes",
            cfg.os_surveilles.len(),
            cfg.max_bones,
            cfg.max_squelettes
        );
        cfg
    }
}

// ── Instantanés ─────────────────────────────────────────────────────────────

/// Un SQUELETTE, et les maillages qui le portent.
#[derive(Debug, Clone)]
pub struct SqueletteSnapshot {
    pub nom: String,
    /// Nombre total de joints du squelette, AVANT plafond. Publié pour qu'un
    /// relevé tronqué se lise comme tronqué.
    pub os_total: usize,
    pub maillages: Vec<MaillageSnapshot>,
    pub os: Vec<BoneSnapshot>,
}

#[derive(Debug, Clone)]
pub struct MaillageSnapshot {
    pub nom: String,
    pub aabb_centre: Vec3,
    pub aabb_demi: Vec3,
    /// Écart entre le centre de l'AABB et le barycentre des joints QUE CE
    /// MAILLAGE utilise. Comparer des choses comparables : l'ancienne version
    /// comparait le museau du chien à ses hanches, et criait forcément.
    pub ecart_m: f32,
}

#[derive(Debug, Clone)]
pub struct BoneSnapshot {
    pub name: String,
    pub depth: u32,
    pub world_translation: Vec3,
    pub local_rotation_euler_deg: Vec3,
    /// Cet os fait-il partie de la liste surveillée ?
    pub surveille: bool,
}

// ── Plugin ──────────────────────────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct BoneTraceTimer {
    pub accum_s: f32,
}

pub struct BoneTracePlugin;

impl Plugin for BoneTracePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(BoneTraceConfig::charger())
            .init_resource::<BoneTraceTimer>()
            .add_systems(Update, write_bone_trace_sensor.in_set(GameSet::Sensors));
    }
}

// ── Relevé ──────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn write_bone_trace_sensor(
    time: Res<Time>,
    cfg: Res<BoneTraceConfig>,
    mut timer: ResMut<BoneTraceTimer>,
    q_skinned: Query<(Entity, &SkinnedMesh, &GlobalTransform, Option<&Aabb>)>,
    q_global: Query<&GlobalTransform>,
    q_transform: Query<&Transform>,
    q_name: Query<&Name>,
    q_parent: Query<&ChildOf>,
) {
    if !cfg.enabled {
        return;
    }
    timer.accum_s += time.delta_secs();
    let interval = (1.0 / cfg.hz.max(0.1)).max(0.05);
    if timer.accum_s < interval {
        return;
    }
    timer.accum_s = 0.0;

    // Regroupement par SQUELETTE. La clé est le premier joint : deux maillages
    // du même personnage partagent la même liste d'entités-joints, donc la même
    // racine. C'est ce qui fait passer le chien de « 8 personnages » à un seul.
    let mut par_squelette: HashMap<Entity, SqueletteSnapshot> = HashMap::default();
    let mut ordre: Vec<Entity> = Vec::new();

    for (mesh_entity, skinned, mesh_gt, mesh_aabb) in q_skinned.iter() {
        let Some(&racine) = skinned.joints.first() else {
            continue;
        };
        if !par_squelette.contains_key(&racine) && ordre.len() >= cfg.max_squelettes {
            continue;
        }

        let entree = par_squelette.entry(racine).or_insert_with(|| {
            ordre.push(racine);
            let (_, nom) = ancetre_nomme(racine, &q_parent, &q_name);
            SqueletteSnapshot {
                nom,
                os_total: skinned.joints.len(),
                maillages: Vec::new(),
                os: releve_des_os(skinned, &cfg, &q_name, &q_global, &q_transform),
            }
        });

        if let Some(aabb) = mesh_aabb {
            let centre = mesh_gt.transform_point(Vec3::from(aabb.center));
            // Barycentre des joints de CE maillage — la seule référence
            // comparable au centre de SON AABB.
            let mut somme = Vec3::ZERO;
            let mut n = 0u32;
            for &j in &skinned.joints {
                if let Ok(gt) = q_global.get(j) {
                    somme += gt.translation();
                    n += 1;
                }
            }
            let bary = if n > 0 { somme / n as f32 } else { centre };
            let (_, nom) = ancetre_nomme(mesh_entity, &q_parent, &q_name);
            entree.maillages.push(MaillageSnapshot {
                nom,
                aabb_centre: centre,
                aabb_demi: Vec3::from(aabb.half_extents),
                ecart_m: centre.distance(bary),
            });
        }
    }

    let snapshots: Vec<SqueletteSnapshot> = ordre
        .iter()
        .filter_map(|e| par_squelette.remove(e))
        .collect();

    // Un maillage dont l'AABB s'éloigne de SES PROPRES joints ne suit plus le
    // squelette. Le seuil vaut pour une comparaison qui a un sens — ce qui
    // n'était pas le cas avant.
    let desync: Vec<(&str, &str, f32)> = snapshots
        .iter()
        .flat_map(|s| {
            s.maillages
                .iter()
                .filter(|m| m.ecart_m > cfg.desync_threshold_m)
                .map(move |m| (s.nom.as_str(), m.nom.as_str(), m.ecart_m))
        })
        .collect();

    ecrire_releve(time.elapsed_secs(), &snapshots, &cfg);
    ecrire_sante(time.elapsed_secs(), &desync, snapshots.len());
}

/// Les os d'un squelette : les surveillés d'abord, le reste ensuite.
fn releve_des_os(
    skinned: &SkinnedMesh,
    cfg: &BoneTraceConfig,
    q_name: &Query<&Name>,
    q_global: &Query<&GlobalTransform>,
    q_transform: &Query<&Transform>,
) -> Vec<BoneSnapshot> {
    let lire = |i: usize, e: Entity, surveille: bool| {
        let name = q_name
            .get(e)
            .map(|n| n.to_string())
            .unwrap_or_else(|_| format!("bone_{i}"));
        let world_translation = q_global
            .get(e)
            .map(|gt| gt.translation())
            .unwrap_or(Vec3::ZERO);
        let (x, y, z) = q_transform
            .get(e)
            .map(|t| t.rotation)
            .unwrap_or(Quat::IDENTITY)
            .to_euler(EulerRot::XYZ);
        BoneSnapshot {
            name,
            depth: i as u32,
            world_translation,
            local_rotation_euler_deg: Vec3::new(x.to_degrees(), y.to_degrees(), z.to_degrees()),
            surveille,
        }
    };

    let mut os = Vec::with_capacity(cfg.max_bones);
    let mut pris: HashSet<usize> = HashSet::default();

    // 1. Les surveillés — hors plafond. Ils sont la raison d'être du relevé :
    //    les faire tomber sous un plafond, c'est reproduire le défaut d'avant.
    for (i, &e) in skinned.joints.iter().enumerate() {
        let Ok(n) = q_name.get(e) else { continue };
        if cfg.os_surveilles.iter().any(|s| s == n.as_str()) {
            os.push(lire(i, e, true));
            pris.insert(i);
        }
    }
    // 2. Le reste, dans l'ordre du fichier, jusqu'au plafond.
    for (i, &e) in skinned.joints.iter().enumerate() {
        if os.len() >= cfg.max_bones + pris.len() {
            break;
        }
        if pris.contains(&i) {
            continue;
        }
        os.push(lire(i, e, false));
    }
    os
}

// ── Écriture ────────────────────────────────────────────────────────────────

fn echappe(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn ecrire_releve(t: f32, snapshots: &[SqueletteSnapshot], cfg: &BoneTraceConfig) {
    let mut j = String::with_capacity(4096);
    let manquants: usize = snapshots
        .iter()
        .map(|s| s.os_total.saturating_sub(s.os.len()))
        .sum();
    j.push_str(&format!(
        "{{\"id\":\"bone_trace\",\"timestamp_secs\":{t:.1},\"squelettes\":{},\"os_ecartes\":{manquants},\"os_surveilles\":[",
        snapshots.len()
    ));
    for (i, n) in cfg.os_surveilles.iter().enumerate() {
        if i > 0 {
            j.push(',');
        }
        j.push_str(&format!("\"{}\"", echappe(n)));
    }
    j.push_str("],\"personnages\":[");
    for (i, s) in snapshots.iter().enumerate() {
        if i > 0 {
            j.push(',');
        }
        j.push_str(&format!(
            "{{\"nom\":\"{}\",\"os_total\":{},\"maillages\":[",
            echappe(&s.nom),
            s.os_total
        ));
        for (mi, m) in s.maillages.iter().enumerate() {
            if mi > 0 {
                j.push(',');
            }
            j.push_str(&format!(
                "{{\"nom\":\"{}\",\"centre\":[{:.2},{:.2},{:.2}],\"demi\":[{:.2},{:.2},{:.2}],\"ecart_m\":{:.3}}}",
                echappe(&m.nom),
                m.aabb_centre.x, m.aabb_centre.y, m.aabb_centre.z,
                m.aabb_demi.x, m.aabb_demi.y, m.aabb_demi.z,
                m.ecart_m
            ));
        }
        j.push_str("],\"os\":[");
        for (bi, b) in s.os.iter().enumerate() {
            if bi > 0 {
                j.push(',');
            }
            j.push_str(&format!(
                "{{\"nom\":\"{}\",\"i\":{},\"surveille\":{},\"monde\":[{:.3},{:.3},{:.3}],\"euler_deg\":[{:.1},{:.1},{:.1}]}}",
                echappe(&b.name),
                b.depth,
                b.surveille,
                b.world_translation.x, b.world_translation.y, b.world_translation.z,
                b.local_rotation_euler_deg.x,
                b.local_rotation_euler_deg.y,
                b.local_rotation_euler_deg.z
            ));
        }
        j.push_str("]}");
    }
    j.push_str("]}");
    let _ = forgia_core::sensor_io::enqueue("forgia_bone_trace.json", j);
}

fn ecrire_sante(t: f32, desync: &[(&str, &str, f32)], squelettes: usize) {
    if squelettes == 0 {
        // Zéro mesuré n'est pas vert, c'est aveugle. Le dire, plutôt que de
        // supprimer le fichier comme si tout allait bien.
        let j = format!(
            r#"{{"id":"bone_trace_health","timestamp_secs":{t:.1},"severity":"info","squelettes":0,"desync":0,"next_step":"AVEUGLE : aucun squelette dans la scene — hors mode a personnage, ou l'avatar n'est pas monte. Ce controle ne mesure rien."}}"#
        );
        let _ = forgia_core::sensor_io::enqueue("forgia_bone_trace_health.json", j);
        return;
    }
    if desync.is_empty() {
        let j = format!(
            r#"{{"id":"bone_trace_health","timestamp_secs":{t:.1},"severity":"ok","squelettes":{squelettes},"desync":0,"next_step":""}}"#
        );
        let _ = forgia_core::sensor_io::enqueue("forgia_bone_trace_health.json", j);
        return;
    }
    let pire = desync
        .iter()
        .fold(0.0_f32, |acc, (_, _, e)| acc.max(*e));
    let liste = desync
        .iter()
        .take(5)
        .map(|(s, m, e)| format!("{{\"perso\":\"{}\",\"maillage\":\"{}\",\"ecart_m\":{e:.2}}}", echappe(s), echappe(m)))
        .collect::<Vec<_>>()
        .join(",");
    let j = format!(
        r#"{{"id":"bone_trace_health","timestamp_secs":{t:.1},"severity":"warn","squelettes":{squelettes},"desync":{},"pire_ecart_m":{pire:.2},"details":[{liste}],"next_step":"MAILLAGE_DECROCHE : l'AABB d'un maillage s'ecarte du barycentre de SES PROPRES joints. Deux causes, dans cet ordre : (1) Bevy ne recalcule pas les AABB des maillages skinnes (limite moteur connue) — c'est ce que corrige bevy_mod_skinned_aabb ; (2) si les AABB SONT a jour, alors la bindpose inverse est fausse (forgia-auto-rig::skinning). Verifier la cause 1 AVANT d'accuser le rig."}}"#,
        desync.len()
    );
    let _ = forgia_core::sensor_io::enqueue("forgia_bone_trace_health.json", j);
}

/// Remonte la chaîne parent jusqu'à une entité nommée. Cap à 10 : un cycle de
/// hiérarchie ferait boucler à l'infini, et un capteur qui gèle le jeu est pire
/// que pas de capteur.
fn ancetre_nomme(start: Entity, q_parent: &Query<&ChildOf>, q_name: &Query<&Name>) -> (Entity, String) {
    let mut current = start;
    for _ in 0..10 {
        if let Ok(name) = q_name.get(current) {
            return (current, name.to_string());
        }
        match q_parent.get(current) {
            Ok(parent) => current = parent.parent(),
            Err(_) => break,
        }
    }
    (start, "sans_nom".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_os_de_la_chaine_de_tir_sont_surveilles_par_defaut() {
        // Ce sont ceux dont le défaut se voit à l'écran — et ceux qui
        // manquaient au relevé le 2026-08-16.
        let c = BoneTraceConfig::default();
        for os in ["RightArm", "RightForeArm", "RightHand", "Spine2"] {
            assert!(
                c.os_surveilles.iter().any(|s| s == os),
                "{os} n'est pas surveillé — il retomberait sous le plafond"
            );
        }
    }

    #[test]
    fn le_genome_livre_se_lit_vraiment() {
        // Le fichier existait depuis story-454 et n'était lu par PERSONNE : son
        // en-tête se disait hot-reloadable, aucun code ne l'ouvrait. Ce test
        // échoue si on le laisse redevenir décoratif.
        let src = std::fs::read_to_string("../../config/genomes/debug_anim.toml")
            .expect("génome de debug anim introuvable");
        assert!(
            src.contains("dbg_bone_trace_enabled"),
            "le fichier ne porte plus les clés que le lecteur attend"
        );
    }

    #[test]
    fn une_liste_vide_au_fichier_n_efface_pas_les_surveilles() {
        // Un `= []` mal placé effacerait la liste et rendrait le relevé
        // silencieusement inutile — le défaut d'origine, à l'identique.
        let defaut = BoneTraceConfig::default();
        assert!(!defaut.os_surveilles.is_empty());
    }

    #[test]
    fn le_plafond_de_squelettes_compte_des_squelettes_pas_des_maillages() {
        // Le chien porte 8 maillages skinnés pour UN squelette. Un plafond à 4
        // squelettes doit donc laisser passer le chien entier — alors que
        // l'ancien plafond « 8 personnages » était déjà saturé par lui seul.
        let c = BoneTraceConfig::default();
        assert!(
            c.max_squelettes >= 2,
            "il faut au moins le joueur et un ennemi"
        );
        assert!(
            c.max_bones >= 20,
            "un plafond trop bas rendrait les non-surveillés inutiles"
        );
    }
}
