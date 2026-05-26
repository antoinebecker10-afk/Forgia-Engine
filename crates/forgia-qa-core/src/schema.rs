//! Infrastructure de versioning de schéma. P1 = v1 partout, pas de migration.
//!
//! Les types sérialisés portent `schema_version: u32`. Lors d'un changement
//! breaking, on incrémente [`crate::SCHEMA_VERSION`] et on implémente
//! [`SchemaMigrate::migrate_from`] pour chaque version antérieure.

use thiserror::Error;

/// Erreurs de migration de schéma.
#[derive(Debug, Error)]
pub enum SchemaError {
    /// Version source inconnue ou plus ancienne que ce que la migration sait gérer.
    #[error("unsupported schema version: {0}")]
    UnsupportedVersion(u32),
    /// Erreur de désérialisation pendant la migration.
    #[error("migration deserialization error: {0}")]
    Deserialize(String),
}

/// Trait à implémenter pour migrer un type entre versions.
///
/// P1 : aucun type ne l'implémente encore (tout est v1). P2+ : à utiliser quand
/// on changera la forme d'un variant `BugCategory` ou d'un champ de `BugContext`.
pub trait SchemaMigrate: Sized {
    /// Tente de désérialiser depuis une version source donnée.
    fn migrate_from(raw: &str, source_version: u32) -> Result<Self, SchemaError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SCHEMA_VERSION;

    #[test]
    fn current_schema_version_is_one() {
        assert_eq!(SCHEMA_VERSION, 1);
    }

    #[test]
    fn schema_error_display() {
        let e = SchemaError::UnsupportedVersion(99);
        assert!(format!("{e}").contains("99"));
    }
}
