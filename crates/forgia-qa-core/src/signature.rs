//! [`BugSignature`] — hash stable pour dédup fenêtre glissante.
//!
//! Stratégie : hash de `(category_discriminant, location_canonical, top_3_stack_frames)`.
//! Utilise `std::hash::DefaultHasher` (SipHash 1-3) — stable dans-process,
//! pas cross-process ni cross-version. C'est volontaire : la signature est
//! dédup-fenêtre 5 min, pas archivage long terme. P7 KG re-hashera à l'insertion
//! si un hash stable cross-version devient nécessaire.

use serde::{Deserialize, Serialize};
use std::hash::{DefaultHasher, Hash, Hasher};

use crate::bug::{BugCategory, BugContext};

/// Hash 64 bits stable dans-process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BugSignature(pub u64);

impl BugSignature {
    /// Calcule la signature à partir de la catégorie et du contexte.
    ///
    /// Le hash inclut : discriminant de la catégorie + représentation textuelle
    /// `Debug` de la catégorie (pour distinguer ex: 2 `FrameTimeSpike` à seuils
    /// différents) + premières 3 frames de stack trace.
    pub fn compute(category: &BugCategory, context: &BugContext) -> Self {
        let mut hasher = DefaultHasher::new();
        // Discriminant via Debug repr (stable tant que le type ne change pas).
        let cat_repr = format!("{category:?}");
        cat_repr.hash(&mut hasher);
        // Top 3 frames pour localisation (si présentes).
        for frame in context.stack_trace.iter().take(3) {
            frame.function.hash(&mut hasher);
            frame.file.hash(&mut hasher);
            frame.line.hash(&mut hasher);
        }
        Self(hasher.finish())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bug::Frame;
    use crate::severity::Severity;

    fn ctx_with_frames(frames: Vec<Frame>) -> BugContext {
        BugContext {
            stack_trace: frames,
            ..Default::default()
        }
    }

    #[test]
    fn stable_across_repeated_calls() {
        let cat = BugCategory::FrameTimeSpike {
            p99_ms: 50.0,
            threshold_ms: 33.0,
        };
        let ctx = ctx_with_frames(vec![Frame {
            function: "physics_step".into(),
            file: "physics.rs".into(),
            line: 42,
        }]);
        let s1 = BugSignature::compute(&cat, &ctx);
        let s2 = BugSignature::compute(&cat, &ctx);
        let s3 = BugSignature::compute(&cat, &ctx);
        assert_eq!(s1, s2);
        assert_eq!(s2, s3);
    }

    #[test]
    fn different_categories_distinct() {
        let ctx = ctx_with_frames(vec![]);
        let a = BugSignature::compute(
            &BugCategory::FrameTimeSpike {
                p99_ms: 50.0,
                threshold_ms: 33.0,
            },
            &ctx,
        );
        let b = BugSignature::compute(
            &BugCategory::AllocationLeak {
                bytes_growth_per_min: 1024,
            },
            &ctx,
        );
        assert_ne!(a, b);
    }

    #[test]
    fn different_thresholds_distinct() {
        let ctx = ctx_with_frames(vec![]);
        let a = BugSignature::compute(
            &BugCategory::FrameTimeSpike {
                p99_ms: 50.0,
                threshold_ms: 33.0,
            },
            &ctx,
        );
        let b = BugSignature::compute(
            &BugCategory::FrameTimeSpike {
                p99_ms: 50.0,
                threshold_ms: 16.6,
            },
            &ctx,
        );
        assert_ne!(a, b);
    }

    #[test]
    fn same_top3_distinct_from_extra_frames() {
        // Frames 4+ ne doivent pas affecter la signature (top_3 only).
        let cat = BugCategory::Panic {
            message: "boom".into(),
            location: "lib.rs:1".into(),
        };
        let frames_3 = vec![
            Frame {
                function: "a".into(),
                file: "x.rs".into(),
                line: 1,
            },
            Frame {
                function: "b".into(),
                file: "x.rs".into(),
                line: 2,
            },
            Frame {
                function: "c".into(),
                file: "x.rs".into(),
                line: 3,
            },
        ];
        let mut frames_5 = frames_3.clone();
        frames_5.push(Frame {
            function: "d".into(),
            file: "x.rs".into(),
            line: 4,
        });
        frames_5.push(Frame {
            function: "e".into(),
            file: "x.rs".into(),
            line: 5,
        });
        let s_top3 = BugSignature::compute(&cat, &ctx_with_frames(frames_3));
        let s_top5 = BugSignature::compute(&cat, &ctx_with_frames(frames_5));
        assert_eq!(s_top3, s_top5);
    }

    #[test]
    fn ron_roundtrip() {
        let s = BugSignature(0xDEAD_BEEF_CAFE_BABE);
        let ron = ron::to_string(&s).unwrap();
        let back: BugSignature = ron::from_str(&ron).unwrap();
        assert_eq!(s, back);
    }

    // Sanity check: hash distribution. Pas une vraie property test, mais 100 catégories
    // distinctes doivent produire 100 signatures distinctes.
    #[test]
    fn no_collision_on_100_distinct() {
        let _unused = Severity::Major; // ensure import lives
        let ctx = ctx_with_frames(vec![]);
        let sigs: std::collections::HashSet<BugSignature> = (0..100)
            .map(|i| {
                BugSignature::compute(
                    &BugCategory::EntityLeak {
                        archetype: format!("Archetype{i}"),
                        growth_rate: i as f32,
                    },
                    &ctx,
                )
            })
            .collect();
        assert_eq!(sigs.len(), 100);
    }
}
