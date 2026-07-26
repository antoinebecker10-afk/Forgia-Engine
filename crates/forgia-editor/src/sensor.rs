//! Observabilité — `forgia2_editor.json`, écrit une fois par seconde.
//!
//! Sans ce capteur, « regarde l'éditeur » n'aurait aucune réponse lisible : on ne
//! saurait pas si le fichier d'édition a bien été écrit, si la bibliothèque a été
//! trouvée, ni combien d'objets la scène contient.
//!
//! Alerte avec action de remédiation (convention next-step) : une écriture en
//! échec est du travail créateur en train de se perdre → `critical`.

use bevy::prelude::*;
use forgia_core::prelude::*;

use crate::library::EditorLibrary;
use crate::persist::SceneEdits;
use crate::select::Selection;
use crate::transform_ops::ActiveOp;
use crate::EditorSession;

const SENSOR_PATH: &str = "forgia2_editor.json";
const SENSOR_PERIOD_SECS: f32 = 1.0;

#[allow(clippy::too_many_arguments)]
pub fn sys_write_editor_sensor(
    time: Res<Time<Real>>,
    mut next_write: Local<f32>,
    game_mode: Res<State<GameMode>>,
    session: Res<EditorSession>,
    edits: Res<SceneEdits>,
    selection: Res<Selection>,
    library: Res<EditorLibrary>,
    op: Res<ActiveOp>,
) {
    let now = time.elapsed_secs();
    if now < *next_write {
        return;
    }
    *next_write = now + SENSOR_PERIOD_SECS;

    let in_hub = matches!(game_mode.get(), GameMode::CastleHub);
    let (severity, next_step) = severity_for_editor(
        session.open,
        edits.last_save_ok,
        edits.dirty(),
        library.scanned,
        library.total,
    );
    let json = format!(
        r#"{{"id":"editor","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{now:.1},"in_hub":{in_hub},"open":{},"snap":"{}","props":{},"overrides":{},"selected":{},"op":"{}","op_axis":"{}","library_scanned":{},"library_assets":{},"dirty":{},"saves":{},"last_save_ok":{}}}"#,
        session.open,
        session.snap.label(),
        edits.props.len(),
        edits.overrides.len(),
        selection.items.len(),
        op.kind.label(),
        op.axis.label(),
        library.scanned,
        library.total,
        edits.dirty(),
        edits.saves,
        edits.last_save_ok,
    );
    let _ = forgia_core::sensor_io::enqueue(SENSOR_PATH, json);
}

/// Sévérité + action de remédiation. Extraite pour être testable sans app Bevy.
fn severity_for_editor(
    open: bool,
    last_save_ok: bool,
    dirty: bool,
    library_scanned: bool,
    library_total: usize,
) -> (&'static str, &'static str) {
    if !last_save_ok {
        return (
            "critical",
            "ecriture castle_hub_edits.json en echec : verifier droits/verrou sur le fichier",
        );
    }
    if open && library_scanned && library_total == 0 {
        return (
            "warn",
            "bibliotheque vide : lancer le jeu depuis la racine du repo (assets/models introuvable)",
        );
    }
    if !open && dirty {
        return (
            "warn",
            "editions non ecrites alors que l'editeur est ferme : rouvrir le Hall pour declencher la sauvegarde",
        );
    }
    ("ok", "-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_and_clean_is_ok() {
        assert_eq!(severity_for_editor(false, true, false, false, 0).0, "ok");
    }

    #[test]
    fn failed_write_is_critical() {
        assert_eq!(severity_for_editor(true, false, true, true, 900).0, "critical");
    }

    #[test]
    fn empty_library_warns_only_once_scanned() {
        assert_eq!(severity_for_editor(true, true, false, false, 0).0, "ok");
        assert_eq!(severity_for_editor(true, true, false, true, 0).0, "warn");
    }

    #[test]
    fn pending_edits_after_close_warn() {
        assert_eq!(severity_for_editor(false, true, true, true, 900).0, "warn");
    }

    #[test]
    fn every_severity_carries_a_next_step() {
        for case in [
            severity_for_editor(true, false, true, true, 900),
            severity_for_editor(true, true, false, true, 0),
            severity_for_editor(false, true, true, true, 900),
        ] {
            assert_ne!(case.1, "-", "une alerte doit dire quoi faire");
        }
    }
}
