//! sensor_health_sensor.rs — Producteur `forgia2_sensor_health.json` (1Hz, meta).
//!
//! Lit les timestamps des forgia2_*.json canoniques et expose un résumé CHK-5 :
//! present / missing / stale.
//!
//! Story-469 — Vague 5 Phase 5b Session C.
//!
//! ## 2026-08-12 — le chien de garde surveillait 13 capteurs sur 128
//!
//! Constaté en session : `forgia2_gamefeel.json` s'est figé à t=218 s alors que la
//! run tournait jusqu'à 419 s, **sans aucune alerte** — il ne figurait pas dans la
//! liste en dur ci-dessous. Un capteur qui cesse silencieusement d'écrire est
//! **pire qu'absent** : il rend une valeur périmée qu'on lit comme actuelle, et
//! tout le diagnostic bâti dessus est faux. C'est la classe de défaut de l'index
//! grepai figé 8 jours et des coupe-circuits ouverts en silence.
//!
//! ### Pourquoi pas un simple seuil sur tous les fichiers
//!
//! Le registre déclare des cadences hétérogènes : `1Hz`, `5s`, `event`, `once`. Un
//! seuil plat ferait hurler le chien sur chaque capteur événementiel — et un
//! capteur menteur qui crie au loup finit ignoré, donc inutile. C'est exactement le
//! piège que `map-design-patterns.md` §13 nomme : **zéro mesuré n'est pas rouge, il
//! est aveugle**.
//!
//! ### Ce qui est surveillé, et ce qui ne l'est pas
//!
//! On ne juge **que ce qu'on a vu tictaquer**. Un capteur devient « vivant » après
//! avoir changé de mtime **au moins deux fois** ; à partir de là, s'il cesse de
//! bouger plus de [`STALLED_THRESHOLD_SECS`], il est signalé **arrêté**. Un capteur
//! écrit une seule fois (`once`) ou par événement n'est jamais jugé — il n'a jamais
//! promis de cadence.
//!
//! Résultat : aucune configuration à tenir à jour, aucun faux positif sur
//! l'événementiel, et un capteur neuf est surveillé sans que personne y pense.

use bevy::prelude::*;
use forgia_core::prelude::GameMode;
use std::collections::HashMap;
use std::time::SystemTime;

/// 12 sensors canoniques attendus (V5 cible — chunks reportable Session D).
const EXPECTED_SENSORS: &[&str] = &[
    "forgia2_health.json",
    "forgia2_rpg_health.json",
    "forgia2_arena.json",
    "forgia2_combat.json",
    "forgia2_perf.json",
    "forgia2_entities.json",
    "forgia2_memory.json",
    "forgia2_assets.json",
    "forgia2_vram.json",
    "forgia2_lifecycle.json",
    "forgia2_watchdog.json",
    "forgia2_audio.json",
    "forgia2_input.json",
    "forgia2_sensor_health.json",
    "forgia2_sensor_io.json",
];
const STALE_THRESHOLD_SECS: u64 = 10;

/// Délai au-delà duquel un capteur **qu'on a vu tictaquer** est déclaré arrêté.
///
/// Généreux à dessein : le registre contient des cadences jusqu'à `5s`, et une
/// frame longue ou un changement de mode peut espacer deux écritures. 60 s ne
/// laisse passer aucun arrêt réel (celui de `gamefeel` durait 200 s) tout en
/// évitant de crier sur un capteur simplement lent.
const STALLED_THRESHOLD_SECS: u64 = 60;

/// Nombre de changements de mtime à partir duquel on considère qu'un capteur a
/// **promis une cadence**. À 1, on ne sait pas s'il est périodique ou `once`.
const LIVE_AFTER_UPDATES: u32 = 2;

/// Ce qu'on sait d'un fichier capteur observé pendant la session.
#[derive(Debug, Clone)]
pub struct Observed {
    pub last_mtime: SystemTime,
    /// Combien de fois le mtime a CHANGÉ depuis le début de la session.
    pub updates: u32,
    /// Quand ce changement a été constaté (horloge murale).
    pub last_change: SystemTime,
}

impl Observed {
    /// Un capteur « vivant » a montré une cadence : il est légitime d'attendre
    /// qu'il continue. Les `once` et les événementiels n'entrent jamais ici.
    pub fn is_live(&self) -> bool {
        self.updates >= LIVE_AFTER_UPDATES
    }
}

/// Mémoire du chien de garde entre deux ticks. **Pas state-scopée** : elle doit
/// survivre aux transitions de mode, sinon chaque changement d'écran remettrait
/// tous les compteurs à zéro et plus rien ne serait jamais « vivant ».
#[derive(Resource, Default, Debug)]
pub struct SensorWatch {
    pub seen: HashMap<String, Observed>,
}

/// Pur — met à jour l'état d'un fichier et dit s'il est arrêté.
///
/// Rend `true` uniquement pour un capteur **vivant** dont le mtime n'a pas bougé
/// depuis plus de `threshold`. Un fichier jamais vu changer deux fois rend
/// toujours `false` : on ne juge pas ce qu'on n'a pas observé tictaquer.
pub fn observe(
    seen: &mut HashMap<String, Observed>,
    path: &str,
    mtime: SystemTime,
    now: SystemTime,
    threshold_secs: u64,
) -> bool {
    match seen.get_mut(path) {
        None => {
            seen.insert(
                path.to_string(),
                Observed {
                    last_mtime: mtime,
                    updates: 1,
                    last_change: now,
                },
            );
            false
        }
        Some(obs) => {
            if obs.last_mtime != mtime {
                obs.last_mtime = mtime;
                obs.updates = obs.updates.saturating_add(1);
                obs.last_change = now;
                return false;
            }
            if !obs.is_live() {
                return false;
            }
            now.duration_since(obs.last_change)
                .map(|d| d.as_secs() > threshold_secs)
                .unwrap_or(false)
        }
    }
}

/// Pur — un fichier capteur à surveiller ? Exclut les artefacts historiques
/// `*.previous.json`, qui ne sont par nature jamais réécrits.
pub fn is_watchable_sensor(file_name: &str) -> bool {
    (file_name.starts_with("forgia2_") || file_name.starts_with("forgia_"))
        && file_name.ends_with(".json")
        && !file_name.contains(".previous.")
}

/// Pur — extrait pour tests headless.
///
/// `stalled` = capteurs qui tictaquaient et se sont tus. Ils passent en `warn`
/// même si le noyau canonique est intact : c'est le cas qui, avant le 2026-08-12,
/// passait totalement inaperçu.
pub fn severity_for_sensor_health(
    missing: usize,
    stale: usize,
    stalled: usize,
) -> (&'static str, &'static str) {
    if missing >= 3 {
        (
            "critical",
            "≥3 sensors missing — observability degraded (verify plugin wiring)",
        )
    } else if stalled > 0 {
        (
            "warn",
            "Un capteur qui tictaquait s'est TU — sa valeur est perimee mais se lit comme actuelle. Voir stalled_paths : ne fonder aucun diagnostic dessus avant d'avoir relance le jeu.",
        )
    } else if missing > 0 || stale > 0 {
        (
            "warn",
            "1-2 sensors missing/stale — producer may not be tick'd",
        )
    } else {
        ("ok", "")
    }
}

fn expected_in_mode(path: &str, mode: GameMode) -> bool {
    match path {
        "forgia2_arena.json" => mode == GameMode::Fps,
        "forgia2_rpg_health.json" => mode == GameMode::Rpg,
        _ => true,
    }
}

pub fn sys_write_sensor_health(
    time: Res<Time>,
    mode: Res<State<GameMode>>,
    mut accum: Local<f32>,
    mut watch: ResMut<SensorWatch>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let now = crate::checks::wall_now();

    // Balayage de TOUS les capteurs présents, pas seulement des canoniques.
    // `metadata` seul, 1 Hz, ~130 fichiers : négligeable, et c'est le prix pour
    // qu'un capteur neuf soit surveillé sans qu'on pense à l'inscrire.
    let mut stalled: Vec<String> = Vec::new();
    let mut watched = 0usize;
    let mut live = 0usize;
    if let Ok(dir) = std::fs::read_dir(".") {
        for entry in dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !is_watchable_sensor(&name) {
                continue;
            }
            watched += 1;
            let Ok(mtime) = entry.metadata().and_then(|m| m.modified()) else {
                continue;
            };
            if observe(&mut watch.seen, &name, mtime, now, STALLED_THRESHOLD_SECS) {
                stalled.push(name.clone());
            }
            if watch.seen.get(&name).is_some_and(|o| o.is_live()) {
                live += 1;
            }
        }
    }
    stalled.sort();
    let mut stale: Vec<&str> = Vec::new();
    let mut missing: Vec<&str> = Vec::new();
    let mut present = 0u32;

    let expected: Vec<_> = EXPECTED_SENSORS
        .iter()
        .copied()
        .filter(|path| expected_in_mode(path, mode.get().clone()))
        .collect();

    for path in &expected {
        match std::fs::metadata(path).and_then(|m| m.modified()) {
            Ok(mtime) => {
                let age = now.duration_since(mtime).map(|d| d.as_secs()).unwrap_or(0);
                if age > STALE_THRESHOLD_SECS {
                    stale.push(path);
                }
                present += 1;
            }
            Err(_) => missing.push(path),
        }
    }

    let (severity, next_step) =
        severity_for_sensor_health(missing.len(), stale.len(), stalled.len());

    let missing_json = serde_json::to_string(&missing).unwrap_or_else(|_| "[]".to_string());
    let stale_json = serde_json::to_string(&stale).unwrap_or_else(|_| "[]".to_string());
    // Borné : sur un arrêt massif (jeu en pause, alt-tab prolongé) la liste
    // exploserait et noierait le fichier. Le COMPTE reste exact, c'est lui qui
    // dit l'ampleur ; les chemins ne servent qu'à désigner par où commencer.
    let stalled_shown: Vec<&String> = stalled.iter().take(12).collect();
    let stalled_json = serde_json::to_string(&stalled_shown).unwrap_or_else(|_| "[]".to_string());

    let json = format!(
        r#"{{"id":"sensor_health","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"expected":{},"present":{},"missing":{},"stale":{},"watched":{},"live":{},"stalled":{},"missing_paths":{},"stale_paths":{},"stalled_paths":{}}}"#,
        time.elapsed_secs(),
        expected.len(),
        present,
        missing.len(),
        stale.len(),
        watched,
        live,
        stalled.len(),
        missing_json,
        stale_json,
        stalled_json,
    );

    if let Err(e) = forgia_core::sensor_io::enqueue("forgia2_sensor_health.json", json) {
        warn!("[forgia-observability] sensor_health sensor write failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::time::Duration;

    #[test]
    fn severity_ok_all_present() {
        assert_eq!(severity_for_sensor_health(0, 0, 0).0, "ok");
    }

    #[test]
    fn severity_warn_one_missing() {
        let (sev, next) = severity_for_sensor_health(1, 0, 0);
        assert_eq!(sev, "warn");
        assert!(next.contains("missing"));
    }

    #[test]
    fn severity_warn_stale_only() {
        assert_eq!(severity_for_sensor_health(0, 2, 0).0, "warn");
    }

    #[test]
    fn severity_critical_three_or_more_missing() {
        let (sev, next) = severity_for_sensor_health(3, 0, 0);
        assert_eq!(sev, "critical");
        assert!(next.contains("degraded"));
    }

    /// LE cas du 2026-08-12 : le noyau canonique est intact, mais un capteur hors
    /// liste s'est tu. Avant, severity restait "ok" et personne ne voyait rien.
    #[test]
    fn severity_warn_quand_un_capteur_sest_taise_meme_si_le_noyau_va_bien() {
        let (sev, next) = severity_for_sensor_health(0, 0, 1);
        assert_eq!(sev, "warn");
        assert!(
            next.contains("perimee"),
            "le next_step doit dire POURQUOI c'est grave : la valeur se lit comme actuelle"
        );
    }

    #[test]
    fn un_capteur_manquant_reste_prioritaire_sur_un_capteur_tu() {
        assert_eq!(severity_for_sensor_health(3, 0, 5).0, "critical");
    }

    // ── observe() : ce qu'on juge, et ce qu'on refuse de juger ──────────────

    fn t(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    /// Un fichier vu UNE seule fois n'a promis aucune cadence : `once` et
    /// événementiels ne doivent jamais declencher d'alerte, meme apres des heures.
    #[test]
    fn un_capteur_vu_une_seule_fois_nest_jamais_juge() {
        let mut seen = HashMap::new();
        assert!(!observe(&mut seen, "forgia2_once.json", t(100), t(100), 60));
        // 10 000 s plus tard, toujours le meme mtime : aucune alerte.
        assert!(!observe(
            &mut seen,
            "forgia2_once.json",
            t(100),
            t(10_100),
            60
        ));
        assert!(!seen["forgia2_once.json"].is_live());
    }

    /// Deux changements = une cadence promise. Le silence devient alors un defaut.
    #[test]
    fn un_capteur_qui_tictaquait_et_se_tait_est_signale() {
        let mut seen = HashMap::new();
        observe(&mut seen, "forgia2_gamefeel.json", t(0), t(0), 60);
        observe(&mut seen, "forgia2_gamefeel.json", t(1), t(1), 60);
        assert!(
            seen["forgia2_gamefeel.json"].is_live(),
            "2 updates = vivant"
        );

        // Sous le seuil : pas encore d'alerte.
        assert!(!observe(
            &mut seen,
            "forgia2_gamefeel.json",
            t(1),
            t(50),
            60
        ));
        // Au-dela : signale. (Le cas reel durait 200 s.)
        assert!(observe(
            &mut seen,
            "forgia2_gamefeel.json",
            t(1),
            t(201),
            60
        ));
    }

    /// Reprendre l'ecriture doit ETEINDRE l'alerte, sinon un hoquet la fige a vie.
    #[test]
    fn un_capteur_qui_reprend_nest_plus_signale() {
        let mut seen = HashMap::new();
        observe(&mut seen, "forgia2_x.json", t(0), t(0), 60);
        observe(&mut seen, "forgia2_x.json", t(1), t(1), 60);
        assert!(observe(&mut seen, "forgia2_x.json", t(1), t(201), 60));
        // Nouveau mtime : le capteur est reparti.
        assert!(!observe(&mut seen, "forgia2_x.json", t(202), t(202), 60));
        assert!(!observe(&mut seen, "forgia2_x.json", t(202), t(230), 60));
    }

    #[test]
    fn les_artefacts_previous_ne_sont_pas_surveilles() {
        assert!(is_watchable_sensor("forgia2_gamefeel.json"));
        assert!(is_watchable_sensor("forgia_bot_ai.json"));
        // Un `.previous` n'est par nature jamais reecrit : le surveiller
        // produirait une alerte permanente et fausse.
        assert!(!is_watchable_sensor("forgia2_crash.previous.json"));
        assert!(!is_watchable_sensor("Cargo.toml"));
        assert!(!is_watchable_sensor("forgia2_perf.json.tmp"));
    }

    /// Le balayage doit couvrir bien plus que les 15 canoniques — c'etait tout le
    /// probleme. Ce test fige l'intention : la liste en dur ne borne PLUS la
    /// surveillance, elle ne definit que le noyau « doit exister ».
    #[test]
    fn la_liste_en_dur_ne_borne_plus_la_surveillance() {
        let hors_liste = "forgia2_gamefeel.json";
        assert!(
            !EXPECTED_SENSORS.contains(&hors_liste),
            "gamefeel n'est pas canonique — c'est justement pour ca qu'il est passe inapercu"
        );
        assert!(
            is_watchable_sensor(hors_liste),
            "…mais il DOIT desormais etre surveille"
        );
    }

    #[test]
    fn expected_sensors_count_is_15() {
        assert_eq!(EXPECTED_SENSORS.len(), 15);
    }

    #[test]
    fn mode_specific_sensors_are_not_expected_outside_their_mode() {
        assert!(!expected_in_mode("forgia2_arena.json", GameMode::Roguelite));
        assert!(expected_in_mode("forgia2_arena.json", GameMode::Fps));
        assert!(!expected_in_mode("forgia2_rpg_health.json", GameMode::Fps));
    }
}
