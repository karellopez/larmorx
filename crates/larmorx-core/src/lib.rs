//! Shared foundation of larmorx (PLAN.md §5).
//!
//! This crate will hold the in-memory `Image` and `Affine` types, physical-space helpers,
//! error types, the thread pool, deterministic parallel reductions, the seeded RNG and
//! progress/cancellation. In phase L0 it only defines the version.
#![forbid(unsafe_code)]

/// Version shared by every larmorx crate, the CLI and the Python package.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::VERSION;

    #[test]
    fn version_is_major_minor_patch() {
        let release = VERSION.split(['-', '+']).next().unwrap();
        let parts: Vec<&str> = release.split('.').collect();
        assert_eq!(parts.len(), 3, "{VERSION}");
        assert!(parts.iter().all(|p| p.parse::<u64>().is_ok()), "{VERSION}");
    }
}
