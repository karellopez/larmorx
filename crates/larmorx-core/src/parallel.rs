// SPDX-License-Identifier: Apache-2.0
//! Running work on an explicit number of threads.
//!
//! Every larmorx entry point takes `n_threads` (CLAUDE.md rule 5): there is no global thread
//! setting and no environment variable. `0` means "all logical CPUs".

use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex, OnceLock};

/// The thread pool could not be created.
#[derive(Debug, thiserror::Error)]
#[error("could not start a pool of {n_threads} threads: {source}")]
pub struct ThreadPoolError {
    pub n_threads: usize,
    #[source]
    source: rayon::ThreadPoolBuildError,
}

/// `n_threads`, with 0 replaced by the number of logical CPUs.
pub fn resolve_threads(n_threads: usize) -> usize {
    if n_threads == 0 {
        std::thread::available_parallelism().map_or(1, NonZeroUsize::get)
    } else {
        n_threads
    }
}

/// Runs `f` inside a pool of exactly `n_threads` threads, so that rayon parallelism within `f`
/// uses that many (never the global pool).
///
/// Pools are created on first use and kept for later calls with the same thread count, so
/// small calls do not pay for starting threads.
pub fn with_threads<R, F>(n_threads: usize, f: F) -> Result<R, ThreadPoolError>
where
    R: Send,
    F: FnOnce() -> R + Send,
{
    Ok(pool(resolve_threads(n_threads))?.install(f))
}

fn pool(n: usize) -> Result<Arc<rayon::ThreadPool>, ThreadPoolError> {
    static POOLS: OnceLock<Mutex<HashMap<usize, Arc<rayon::ThreadPool>>>> = OnceLock::new();
    let mut pools = POOLS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if let Some(p) = pools.get(&n) {
        return Ok(Arc::clone(p));
    }
    let p = Arc::new(
        rayon::ThreadPoolBuilder::new()
            .num_threads(n)
            .thread_name(move |i| format!("larmorx-{n}-{i}"))
            .build()
            .map_err(|source| ThreadPoolError {
                n_threads: n,
                source,
            })?,
    );
    pools.insert(n, Arc::clone(&p));
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pools_have_the_requested_size() {
        assert_eq!(with_threads(3, rayon::current_num_threads).unwrap(), 3);
        assert_eq!(with_threads(1, rayon::current_num_threads).unwrap(), 1);
        assert_eq!(with_threads(3, rayon::current_num_threads).unwrap(), 3); // cached pool
        assert_eq!(resolve_threads(5), 5);
        assert!(resolve_threads(0) >= 1);
    }
}
