//! Forgia QA Core — bus typé fondateur de la refonte QA (story-391 P1).
//!
//! Cette crate fournit les types et l'infrastructure unique sur lesquels reposent
//! tous les piliers QA P2-P8 :
//!
//! - [`BugReport`] : structure typée canonique pour tout bug détecté
//! - [`BugBus`] / [`MetricBus`] : Resources Bevy fédératrices (Bevy 0.18 `Message` API)
//! - [`BugSink`] : trait pour persistance (`FileSink` RON, `StdoutSink`, `NullSink`)
//! - [`SlidingWindowDedup`] : dédup par [`BugSignature`] sur fenêtre glissante (default 5 min)
//! - [`MetricSample`] : échantillon numérique pour télémétrie continue
//!
//! ## Feature gates
//!
//! - `qa-runtime` (off par défaut release) : active la collecte runtime. Sans cette
//!   feature, [`drain_bug_messages_to_bus`] est compilée en no-op (validation
//!   `cargo asm` critère sortie P1 §7).
//! - `qa-record` : active les sinks fichier sur disque.
//! - `qa-trace` : active les spans Tracy (réservé P2+).
//!
//! ## Conventions
//!
//! - Sérialisation : RON pour humains, MessagePack à venir P1 W6 pour binaire.
//! - Schema : tout type sérialisé porte `schema_version: u32`. Migrations via
//!   [`schema::SchemaMigrate`] trait.
//! - GameSet : aucun system de cette crate ne touche à la chaîne 7 étapes Forgia.
//!   Les drains tournent dans `PreUpdate` via [`QaCoreSet::Drain`].
//!
//! Voir `docs/qa/PLAN_P1.md` pour le plan détaillé.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod bug;
pub mod bus;
pub mod dedup;
pub mod metric;
pub mod plugin;
pub mod schema;
pub mod severity;
pub mod signature;
pub mod sink;
pub mod source;
pub mod tr_id;

pub use bug::{
    BugCategory, BugContext, BugId, BugReport, BugStatus, EngineEvent, Frame,
    SystemMetricsSnapshot, VisualAnomalyKind,
};
pub use bus::{BugBus, MetricBus};
pub use dedup::SlidingWindowDedup;
pub use metric::MetricSample;
pub use plugin::{ForgiaQaCorePlugin, QaCoreSet, drain_bug_messages_to_bus};
pub use schema::SchemaMigrate;
pub use severity::Severity;
pub use signature::BugSignature;
pub use sink::{BugSink, FileSink, NullSink, SerFormat, SinkError, StdoutSink};
pub use source::DetectionSource;
pub use tr_id::TrId;

/// Version courante du schéma sérialisé. À incrémenter lors d'une migration breaking.
/// Voir [`schema::SchemaMigrate`].
pub const SCHEMA_VERSION: u32 = 1;
