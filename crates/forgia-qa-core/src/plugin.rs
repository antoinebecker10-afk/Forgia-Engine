//! [`ForgiaQaCorePlugin`] — branchement Bevy 0.18 (Message API).
//!
//! Émetteurs pousseront un `BugReport` via `MessageWriter<BugReport>` et un
//! `MetricSample` via `MessageWriter<MetricSample>`. Ce plugin :
//!
//! 1. Enregistre [`crate::BugBus`] et [`crate::MetricBus`] comme Resources.
//! 2. Enregistre `BugReport` et `MetricSample` comme Messages Bevy.
//! 3. Ajoute les systems de drain dans `PreUpdate`, gated derrière la feature
//!    `qa-runtime`.
//!
//! Sans la feature `qa-runtime`, les fonctions de drain sont compilées en no-op
//! (validation `cargo asm` critère sortie P1 §7).

use bevy::app::{App, Plugin, PreUpdate};
use bevy::ecs::message::{Message, MessageReader};
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy::ecs::system::ResMut;

use crate::bug::BugReport;
use crate::bus::{BugBus, MetricBus};
use crate::metric::MetricSample;

// `BugReport` et `MetricSample` deviennent des Messages Bevy via les impls explicites.
// Bevy 0.18 a une derive macro mais on peut aussi impl le trait à la main pour les
// types vivant dans une crate qui ne dépend pas de `bevy::derive`.
impl Message for BugReport {}
impl Message for MetricSample {}

/// SystemSet pour ordonner les drains entre eux. Hors `GameSet` Forgia : aucun
/// conflit avec la chaîne 7 étapes (Input→Movement→Physics→Camera→Combat→Effects→UI).
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QaCoreSet {
    /// Draine `Messages<BugReport>` et `Messages<MetricSample>` vers les bus.
    Drain,
}

/// Plugin Bevy à ajouter au App du jeu (`forgia-game/src/main.rs` en P2 wiring).
#[derive(Debug, Default)]
pub struct ForgiaQaCorePlugin;

impl Plugin for ForgiaQaCorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BugBus>()
            .init_resource::<MetricBus>()
            .add_message::<BugReport>()
            .add_message::<MetricSample>()
            .add_systems(
                PreUpdate,
                (drain_bug_messages_to_bus, drain_metric_messages_to_bus)
                    .chain()
                    .in_set(QaCoreSet::Drain),
            );

        // story-622 inc.3 — sous `qa-record`, on remplace le NullSink par défaut par
        // un FileSink RON : chaque BugReport distinct persiste en
        // `target/forgia_qa/inbox/<id>.bug.ron` (lisible/triable). Sans la feature,
        // le bus garde le NullSink (collecte = compteurs seulement).
        #[cfg(feature = "qa-record")]
        app.add_systems(bevy::app::Startup, install_record_sink);
    }
}

/// Installe le `FileSink` RON au démarrage (remplace le `NullSink` par défaut).
/// Best-effort : si la création du dossier échoue, le bus reste fonctionnel
/// (NullSink conservé) — la collecte QA ne doit jamais casser le boot du jeu.
#[cfg(feature = "qa-record")]
fn install_record_sink(mut bus: ResMut<BugBus>) {
    use crate::sink::FileSink;
    match FileSink::default_inbox() {
        Ok(sink) => {
            bus.set_sinks(vec![Box::new(sink)]);
            bevy::log::info!("[qa-core] FileSink RON actif → target/forgia_qa/inbox/");
        }
        Err(e) => {
            bevy::log::warn!("[qa-core] FileSink init échoué ({e}) — NullSink conservé");
        }
    }
}

// ----------------------------------------------------------------------------
// Drains — gated derrière `qa-runtime`
// ----------------------------------------------------------------------------

/// Drain `Messages<BugReport>` vers [`BugBus`]. No-op sans `qa-runtime`.
#[cfg(feature = "qa-runtime")]
pub fn drain_bug_messages_to_bus(mut reader: MessageReader<BugReport>, mut bus: ResMut<BugBus>) {
    for report in reader.read() {
        // Ignore les erreurs sink ici (best-effort, on ne casse pas le frame ECS).
        // En P2, un sensor sensor_health.json comptera les sink failures.
        let _ = bus.ingest(report.clone());
    }
}

/// No-op release sans `qa-runtime`. Tous arguments inutilisés.
#[cfg(not(feature = "qa-runtime"))]
pub fn drain_bug_messages_to_bus(_reader: MessageReader<BugReport>, _bus: ResMut<BugBus>) {}

/// Drain `Messages<MetricSample>` vers [`MetricBus`]. No-op sans `qa-runtime`.
#[cfg(feature = "qa-runtime")]
fn drain_metric_messages_to_bus(
    mut reader: MessageReader<MetricSample>,
    mut bus: ResMut<MetricBus>,
) {
    for sample in reader.read() {
        bus.ingest(sample.clone());
    }
}

#[cfg(not(feature = "qa-runtime"))]
fn drain_metric_messages_to_bus(_reader: MessageReader<MetricSample>, _bus: ResMut<MetricBus>) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_builds_without_panic() {
        let mut app = App::new();
        app.add_plugins(ForgiaQaCorePlugin);
        // Les Resources doivent être enregistrées.
        assert!(app.world().contains_resource::<BugBus>());
        assert!(app.world().contains_resource::<MetricBus>());
    }

    #[test]
    #[cfg(feature = "qa-runtime")]
    fn drain_pulls_message_into_bus() {
        use crate::bug::BugCategory;
        use crate::severity::Severity;
        use crate::source::DetectionSource;
        let mut app = App::new();
        app.add_plugins(ForgiaQaCorePlugin);
        let report = BugReport::new(
            BugCategory::Panic {
                message: "test".into(),
                location: "x".into(),
            },
            Severity::Blocker,
            DetectionSource::UnitTest {
                test_name: "drain_pulls".into(),
            },
        );
        app.world_mut().write_message(report);
        // PreUpdate s'exécute lors de app.update().
        app.update();
        let bus = app.world().resource::<BugBus>();
        assert!(bus.total_ingested >= 1);
    }
}
