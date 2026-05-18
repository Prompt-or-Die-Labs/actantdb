//! Embedding-space compatibility checks.
//!
//! Vectors produced by different providers (or different model versions of
//! the same provider) live in incompatible coordinate systems. The registry
//! and downstream consumers use [`cross_space_check`] before mixing
//! embeddings — e.g. before computing cosine similarity across two stores.

use thiserror::Error;

/// Cross-space rejection error.
#[derive(Debug, Error)]
pub enum SpaceError {
    /// Two embedders disagree on `provider()`; mixing them is invalid until
    /// an explicit cross-space transform is registered.
    #[error("cross-space mismatch: lhs provider={lhs:?} rhs provider={rhs:?}")]
    ProviderMismatch {
        /// Left-hand provider id.
        lhs: String,
        /// Right-hand provider id.
        rhs: String,
    },
}

/// Reject if two providers are different. Returns `Ok(())` on a match.
///
/// This is a *strict* check — two providers that happen to share a dimension
/// are still rejected. The retrieval planner is expected to insert an
/// explicit cross-space adapter when intentional.
pub fn cross_space_check(lhs: &str, rhs: &str) -> Result<(), SpaceError> {
    if lhs == rhs {
        Ok(())
    } else {
        Err(SpaceError::ProviderMismatch {
            lhs: lhs.into(),
            rhs: rhs.into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_is_ok() {
        assert!(cross_space_check("hash", "hash").is_ok());
    }

    #[test]
    fn mismatch_is_err() {
        let err = cross_space_check("hash", "openai").unwrap_err();
        match err {
            SpaceError::ProviderMismatch { lhs, rhs } => {
                assert_eq!(lhs, "hash");
                assert_eq!(rhs, "openai");
            }
        }
    }
}
