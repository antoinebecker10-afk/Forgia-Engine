//! Types canoniques [`BugReport`], [`BugCategory`], [`BugContext`], [`BugStatus`].
//!
//! Schéma stable v1, couvrant 100% des catégories de bugs réels Forgia observés
//! sur les 200 derniers commits (cf. `docs/qa/AUDIT.md` §3).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::PathBuf;
use ulid::Ulid;

use crate::severity::Severity;
use crate::signature::BugSignature;
use crate::source::DetectionSource;
use crate::tr_id::TrId;
use crate::SCHEMA_VERSION;

/// Identifiant unique de bug. ULID = lexicographiquement triable, 128 bits, dédup-friendly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BugId(pub Ulid);

impl BugId {
    /// Génère un nouveau [`BugId`] basé sur l'horloge système.
    pub fn new() -> Self {
        Self(Ulid::new())
    }
}

impl Default for BugId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for BugId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Statut de cycle de vie d'un bug.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum BugStatus {
    /// Vient d'être détecté, jamais trié.
    New,
    /// Trié, en attente d'action.
    Triaged,
    /// Nécessite intervention humaine (ex: décision design).
    NeedsHuman,
    /// Corrigé, vérifié non-reproductible.
    Fixed,
    /// Volontairement non corrigé (raison à fournir).
    WontFix {
        /// Justification.
        reason: String,
    },
    /// Bug `Fixed` qui réapparaît avec même signature (P8 lifecycle).
    Regression {
        /// `BugId` de la résolution précédente.
        previous_id: BugId,
    },
}

/// Catégorie typée. Couvre 100% des bugs Forgia observés (cf. AUDIT.md §3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "category", rename_all = "snake_case")]
#[allow(missing_docs)] // variants self-explanatory, documented as group above
pub enum BugCategory {
    LogicInvariantViolation {
        tr_id: String,
        detail: String,
    },
    StateDesync {
        expected: String,
        actual: String,
    },
    EventOrderingViolation {
        detail: String,
    },
    FrameTimeSpike {
        p99_ms: f32,
        threshold_ms: f32,
    },
    SystemOverbudget {
        system: String,
        budget_ms: f32,
        actual_ms: f32,
        tr_budget_id: String,
    },
    AllocationLeak {
        bytes_growth_per_min: u64,
    },
    EntityLeak {
        archetype: String,
        growth_rate: f32,
    },
    VramOverbudget {
        used_mb: u64,
        budget_mb: u64,
    },
    AssetLoadFailure {
        path: String,
        reason: String,
    },
    ShaderCompilationError {
        detail: String,
    },
    VisualAnomaly {
        kind: VisualAnomalyKind,
    },
    PbrMaterialViolation {
        entity_repr: String,
        reason: String,
    },
    PhysicsExplosion {
        entity_repr: String,
        velocity: f32,
    },
    Stuck {
        entity_repr: String,
        duration_s: f32,
    },
    OutOfBounds {
        entity_repr: String,
        x: f32,
        y: f32,
        z: f32,
    },
    TerrainAnomaly {
        kind: String,
        chunk_x: i32,
        chunk_z: i32,
    },
    NonDeterminism {
        divergence_frame: u64,
        field: String,
    },
    Panic {
        message: String,
        location: String,
    },
    ConfigSchemaMismatch {
        file: String,
        field: String,
    },
    SaveFileCorruption {
        path: String,
        reason: String,
    },
    StabilityLockViolation {
        lock_id: String,
        detail: String,
    },
}

/// Sous-classification des [`BugCategory::VisualAnomaly`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum VisualAnomalyKind {
    ZFighting,
    MissingTexture,
    PlaceholderTexture,
    LowResolutionTexture,
    Stretching,
    Flicker,
    DualWriterArtifact,
    BloomBlast,
    DuskSilhouetteBroken,
}

/// Trame de stack-trace simplifiée. `function:file:line`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frame {
    /// Nom de la fonction.
    pub function: String,
    /// Fichier source.
    pub file: String,
    /// Numéro de ligne.
    pub line: u32,
}

/// Événement moteur récent, capturé dans le ring buffer [`BugContext::recent_events`].
/// Format minimal P1 — sera étendu en P2 avec plus de variants typés.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineEvent {
    /// Frame moteur où l'événement s'est produit.
    pub frame: u64,
    /// Catégorie textuelle (ex: "input.key_pressed", "ecs.spawn", "physics.collision").
    pub kind: String,
    /// Détail libre (sera typé en P2).
    pub detail: String,
}

/// Snapshot agrégé des métriques système au moment de la détection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SystemMetricsSnapshot {
    /// FPS instantané.
    pub fps: f32,
    /// Frame time ms.
    pub frame_time_ms: f32,
    /// Nombre d'entités vivantes.
    pub entity_count: u32,
    /// Mémoire process estimée (Mo).
    pub process_mem_mb: u64,
    /// VRAM utilisée (Mo).
    pub vram_used_mb: u64,
}

/// Contexte complet permettant la reproduction (P4 replay).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BugContext {
    /// SHA git + flag dirty.
    pub build_hash: String,
    /// Seed RNG global au démarrage.
    pub seed: u64,
    /// Frame moteur au moment de la détection.
    pub frame: u64,
    /// Chemin vers snapshot ECS sérialisé (MessagePack en P2+).
    pub world_snapshot_path: Option<PathBuf>,
    /// Chemin vers replay des inputs (BITT-style en P4).
    pub input_replay_path: Option<PathBuf>,
    /// Capture d'écran au moment de la détection.
    pub screenshot_path: Option<PathBuf>,
    /// Stack trace (vide si pas applicable, ex: anomalie statistique).
    pub stack_trace: Vec<Frame>,
    /// Ring buffer des 256 derniers événements.
    pub recent_events: VecDeque<EngineEvent>,
    /// Métriques agrégées.
    pub system_metrics: SystemMetricsSnapshot,
    /// Mode applicatif (Play / Build / Edit).
    pub app_mode: String,
    /// Carte active (Dungeon / Terrain / Imported / None).
    pub map_id: Option<String>,
}

impl Default for BugContext {
    fn default() -> Self {
        Self {
            build_hash: String::from("unknown"),
            seed: 0,
            frame: 0,
            world_snapshot_path: None,
            input_replay_path: None,
            screenshot_path: None,
            stack_trace: Vec::new(),
            recent_events: VecDeque::new(),
            system_metrics: SystemMetricsSnapshot::default(),
            app_mode: String::from("unknown"),
            map_id: None,
        }
    }
}

/// Rapport canonique unifié — la seule sortie typée du pipeline QA.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BugReport {
    /// Identifiant unique de cette occurrence.
    pub id: BugId,
    /// Hash stable pour dédup. Voir [`crate::signature::BugSignature`].
    pub signature: BugSignature,
    /// Catégorie typée.
    pub category: BugCategory,
    /// Sévérité.
    pub severity: Severity,
    /// Source de détection.
    pub source: DetectionSource,
    /// Contexte de reproduction.
    pub context: BugContext,
    /// Première fois vu (UTC).
    pub first_seen: DateTime<Utc>,
    /// Dernière fois vu (UTC).
    pub last_seen: DateTime<Utc>,
    /// Compteur d'occurrences (>1 si dédup a aggrégé).
    pub occurrences: u32,
    /// Statut courant.
    pub status: BugStatus,
    /// Refs vers `tr-registry.yaml` (peut être vide).
    pub tr_refs: Vec<TrId>,
    /// Version du schéma sérialisé. Toujours [`crate::SCHEMA_VERSION`] en écriture.
    pub schema_version: u32,
}

impl BugReport {
    /// Construit un rapport "frais" : `id` neuf, `first_seen = last_seen = now`,
    /// `occurrences = 1`, `status = New`, `tr_refs` vides, `schema_version = SCHEMA_VERSION`.
    /// La signature est calculée à partir de la catégorie + emplacement texte +
    /// stack trace fournis dans `context`.
    pub fn new(category: BugCategory, severity: Severity, source: DetectionSource) -> Self {
        let context = BugContext::default();
        Self::with_context(category, severity, source, context)
    }

    /// Comme [`Self::new`] mais avec un [`BugContext`] explicite.
    pub fn with_context(
        category: BugCategory,
        severity: Severity,
        source: DetectionSource,
        context: BugContext,
    ) -> Self {
        let now = Utc::now();
        let signature = BugSignature::compute(&category, &context);
        Self {
            id: BugId::new(),
            signature,
            category,
            severity,
            source,
            context,
            first_seen: now,
            last_seen: now,
            occurrences: 1,
            status: BugStatus::New,
            tr_refs: Vec::new(),
            schema_version: SCHEMA_VERSION,
        }
    }

    /// Ajoute une référence TR-ID à ce report.
    pub fn with_tr_ref(mut self, tr_id: TrId) -> Self {
        self.tr_refs.push(tr_id);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_report() -> BugReport {
        BugReport::new(
            BugCategory::FrameTimeSpike {
                p99_ms: 50.0,
                threshold_ms: 33.0,
            },
            Severity::Major,
            DetectionSource::TelemetryAnomaly {
                metric: "frame_time_ms".into(),
                score: 4.2,
            },
        )
    }

    #[test]
    fn new_initializes_canonical_fields() {
        let r = sample_report();
        assert_eq!(r.occurrences, 1);
        assert_eq!(r.schema_version, SCHEMA_VERSION);
        assert!(matches!(r.status, BugStatus::New));
        assert!(r.tr_refs.is_empty());
        assert_eq!(r.first_seen, r.last_seen);
    }

    #[test]
    fn with_tr_ref_appends() {
        let r = sample_report().with_tr_ref(TrId::new("TR-budget-frame-001").unwrap());
        assert_eq!(r.tr_refs.len(), 1);
        assert_eq!(r.tr_refs[0].as_str(), "TR-budget-frame-001");
    }

    #[test]
    fn ron_roundtrip_full() {
        let r = sample_report();
        let s = ron::ser::to_string(&r).expect("serialize");
        let back: BugReport = ron::from_str(&s).expect("deserialize");
        assert_eq!(r.id, back.id);
        assert_eq!(r.signature, back.signature);
        assert_eq!(r.severity, back.severity);
        assert_eq!(r.schema_version, back.schema_version);
    }

    #[test]
    fn bug_id_unique_across_calls() {
        let a = BugId::new();
        let b = BugId::new();
        assert_ne!(a, b);
    }
}
