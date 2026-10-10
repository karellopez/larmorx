// SPDX-License-Identifier: Apache-2.0
//! The error type of every filter in this crate.

use larmorx_core::parallel::ThreadPoolError;

/// A filter could not run.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FilterError {
    /// The arguments are invalid. Where ITK throws an exception for the same arguments, the
    /// message is ITK's.
    #[error("{0}")]
    Invalid(String),
    /// Two inputs that must have the same number of voxels do not.
    #[error("{what}: {actual} voxels, expected {expected}")]
    SizeMismatch {
        what: &'static str,
        actual: usize,
        expected: usize,
    },
    #[error(transparent)]
    Threads(#[from] ThreadPoolError),
}

impl FilterError {
    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        FilterError::Invalid(message.into())
    }
}
