// SPDX-License-Identifier: Apache-2.0
//! Allocations whose size comes from the input: an output grid, a padding, a resampled image.
//!
//! Rust aborts the process when an allocation fails, which in the Python extension takes the
//! interpreter down with it. ITK throws `itk::MemoryAllocationError` instead ("Failed to
//! allocate memory for image."), which the ANTs programs do not catch: they abort. Where an
//! input can ask for more memory than the machine has (a grid in metres resampled to
//! millimetres, a spline distance of 200 mm on micrometre voxels), larmorx allocates through
//! [`filled`] and refuses with ITK's message (D13 class (a): a crash defines nothing; larmorx
//! exits 1 in both modes). Found by differential random testing (2026-10-25).

use std::cell::Cell;
use std::sync::OnceLock;

/// The memory larmorx plans a computation for: a limit given to this thread's computations
/// ([`with_memory_limit`]), else `LARMORX_MEMORY_LIMIT` (bytes, or with a binary suffix: `512M`,
/// `64G`, `2T`) when set, else the machine's physical memory, else [`DEFAULT_LIMIT`].
///
/// An allocation larger than the address space fails at once ([`filled`]), but one that fits it
/// and not the machine succeeds and fills the memory while it is written (Linux overcommits):
/// the machine swaps until the process is stopped. Where an input can make a computation need
/// more memory than any machine has (ANTS-162: N4's lattice and padding, a resampled grid,
/// for an image whose header gives its voxels in metres or micrometres), larmorx computes the
/// bytes it will need before it allocates, and stops at once with a clear error if they exceed
/// this limit (D13: a crash is never reproduced by default, and exhausting the machine is
/// one); `reproduce` attempts it as the original does.
pub const MEMORY_LIMIT_ENV: &str = "LARMORX_MEMORY_LIMIT";

/// The limit where the machine's memory cannot be read (fail-safe: what no machine larmorx runs
/// on has; a plan above it is refused at once with its error).
pub const DEFAULT_LIMIT: u64 = 1 << 40;

/// Where a [`MemoryLimit`] comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimitSource {
    /// Given to this thread's computations ([`with_memory_limit`]).
    Given,
    /// `LARMORX_MEMORY_LIMIT`.
    Environment,
    /// The machine's physical memory.
    Physical,
    /// [`DEFAULT_LIMIT`]: the machine's memory could not be read.
    Default,
}

/// The memory a computation may plan for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryLimit {
    pub bytes: u64,
    pub source: LimitSource,
}

impl std::fmt::Display for MemoryLimit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.source {
            LimitSource::Given => write!(
                f,
                "the {} given to this computation",
                gib(self.bytes as f64)
            ),
            LimitSource::Default => write!(
                f,
                "larmorx's default of {} (this machine's memory could not be read; \
                 {MEMORY_LIMIT_ENV} sets a limit)",
                gib(self.bytes as f64)
            ),
            LimitSource::Environment => {
                write!(f, "the {} of {MEMORY_LIMIT_ENV}", gib(self.bytes as f64))
            }
            LimitSource::Physical => write!(
                f,
                "this machine's {} of memory ({MEMORY_LIMIT_ENV} sets another limit)",
                gib(self.bytes as f64)
            ),
        }
    }
}

/// `1.55e+03 GiB`, `246 GiB`, `0.25 GiB`: three significant digits.
pub fn gib(bytes: f64) -> String {
    let g = bytes / (1u64 << 30) as f64;
    if g >= 1000.0 {
        format!("{g:.3e} GiB")
    } else {
        let digits = if g >= 100.0 {
            0
        } else if g >= 10.0 {
            1
        } else {
            2
        };
        format!("{g:.digits$} GiB")
    }
}

/// `"64G"`, `"512 MiB"`, `"1e9"`: bytes (binary suffixes K, M, G, T, with or without `iB`/`B`).
pub fn parse_bytes(text: &str) -> Option<u64> {
    let t = text.trim();
    let split = t
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == 'e' || c == 'E' || c == '+'))
        .unwrap_or(t.len());
    let (number, unit) = t.split_at(split);
    let value: f64 = number.trim().parse().ok()?;
    let unit = unit.trim().to_ascii_uppercase();
    let unit = unit.trim_end_matches('B').trim_end_matches('I');
    let scale = match unit {
        "" => 1u64,
        "K" => 1 << 10,
        "M" => 1 << 20,
        "G" => 1 << 30,
        "T" => 1 << 40,
        _ => return None,
    };
    let bytes = value * scale as f64;
    (bytes.is_finite() && bytes >= 0.0).then_some(bytes as u64)
}

/// The machine's physical memory (Linux: `/proc/meminfo`; macOS: `sysctl hw.memsize`;
/// Windows: `GlobalMemoryStatusEx`), or `None` where it cannot be read. Read once.
pub fn physical_memory() -> Option<u64> {
    static MEMORY: OnceLock<Option<u64>> = OnceLock::new();
    *MEMORY.get_or_init(read_physical_memory)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn read_physical_memory() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let line = text.lines().find(|l| l.starts_with("MemTotal:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kib * 1024)
}

#[cfg(target_os = "macos")]
fn read_physical_memory() -> Option<u64> {
    let out = std::process::Command::new("/usr/sbin/sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok()?;
    String::from_utf8(out.stdout).ok()?.trim().parse().ok()
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn read_physical_memory() -> Option<u64> {
    // Audited (CLAUDE.md rule 5): one call of a kernel32 function every Windows process links,
    // on a structure of plain integers this function owns; nothing is kept. A PowerShell query
    // took over 5 s on the windows-arm64 runner (CI, 2026-10-26).
    #[repr(C)]
    #[allow(dead_code)] // MEMORYSTATUSEX's layout: only the total is read
    struct MemoryStatusEx {
        length: u32,
        memory_load: u32,
        total_phys: u64,
        avail_phys: u64,
        total_page_file: u64,
        avail_page_file: u64,
        total_virtual: u64,
        avail_virtual: u64,
        avail_extended_virtual: u64,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GlobalMemoryStatusEx(buffer: *mut MemoryStatusEx) -> i32;
    }
    let mut status = MemoryStatusEx {
        length: std::mem::size_of::<MemoryStatusEx>() as u32,
        memory_load: 0,
        total_phys: 0,
        avail_phys: 0,
        total_page_file: 0,
        avail_page_file: 0,
        total_virtual: 0,
        avail_virtual: 0,
        avail_extended_virtual: 0,
    };
    // SAFETY: `status` is a valid, writable MEMORYSTATUSEX with `dwLength` set, as the API
    // requires; the call writes only within it.
    let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
    (ok != 0 && status.total_phys > 0).then_some(status.total_phys)
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    windows
)))]
fn read_physical_memory() -> Option<u64> {
    None
}

thread_local! {
    static GIVEN: Cell<Option<u64>> = const { Cell::new(None) };
}

/// Runs `f` with `bytes` as the memory limit of this thread's plans, before
/// [`MEMORY_LIMIT_ENV`] and the machine's memory: a caller that knows its budget, and the tests
/// of the guards, which then depend on no machine (2026-10-26). The previous limit comes back
/// when `f` returns or panics.
pub fn with_memory_limit<R>(bytes: u64, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<u64>);
    impl Drop for Restore {
        fn drop(&mut self) {
            GIVEN.with(|g| g.set(self.0));
        }
    }
    let _restore = Restore(GIVEN.with(|g| g.replace(Some(bytes))));
    f()
}

/// The memory larmorx may plan for: the limit given to this thread ([`with_memory_limit`]),
/// else [`MEMORY_LIMIT_ENV`], else the physical memory, else [`DEFAULT_LIMIT`] (fail-safe: a
/// guard always has a limit).
pub fn memory_limit() -> MemoryLimit {
    if let Some(bytes) = GIVEN.with(Cell::get) {
        return MemoryLimit {
            bytes,
            source: LimitSource::Given,
        };
    }
    if let Some(bytes) = std::env::var(MEMORY_LIMIT_ENV)
        .ok()
        .as_deref()
        .and_then(parse_bytes)
    {
        return MemoryLimit {
            bytes,
            source: LimitSource::Environment,
        };
    }
    match physical_memory() {
        Some(bytes) => MemoryLimit {
            bytes,
            source: LimitSource::Physical,
        },
        None => MemoryLimit {
            bytes: DEFAULT_LIMIT,
            source: LimitSource::Default,
        },
    }
}

/// `Err(limit)` when a computation that needs `needed` bytes exceeds the [`memory_limit`].
/// Below 1 GiB, with no limit given or set, the machine's memory is not even read (every
/// machine larmorx runs on has it).
pub fn check_plan(needed: f64) -> Result<(), MemoryLimit> {
    let chosen = GIVEN.with(Cell::get).is_some() || std::env::var_os(MEMORY_LIMIT_ENV).is_some();
    if !chosen && needed < (1u64 << 30) as f64 {
        return Ok(());
    }
    let limit = memory_limit();
    if needed > limit.bytes as f64 {
        Err(limit)
    } else {
        Ok(())
    }
}

/// An allocation the machine cannot make.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("Failed to allocate memory for image ({elements} values of {element_bytes} bytes)")]
pub struct AllocError {
    pub elements: usize,
    pub element_bytes: usize,
}

/// `vec![value; n]`, or an [`AllocError`] where the memory cannot be had.
pub fn filled<T: Clone>(n: usize, value: T) -> Result<Vec<T>, AllocError> {
    let error = || AllocError {
        elements: n,
        element_bytes: std::mem::size_of::<T>(),
    };
    n.checked_mul(std::mem::size_of::<T>())
        .filter(|&bytes| bytes <= isize::MAX as usize)
        .ok_or_else(error)?;
    let mut v = Vec::new();
    v.try_reserve_exact(n).map_err(|_| error())?;
    v.resize(n, value);
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_allocations_are_filled() {
        assert_eq!(filled(4, 1.5f32).unwrap(), vec![1.5; 4]);
        assert!(filled(0, 0u8).unwrap().is_empty());
    }

    #[test]
    fn byte_counts_and_limits() {
        assert_eq!(parse_bytes("512"), Some(512));
        assert_eq!(parse_bytes("64G"), Some(64 << 30));
        assert_eq!(parse_bytes(" 1.5 GiB "), Some(3 << 29));
        assert_eq!(parse_bytes("2t"), Some(2 << 40));
        assert_eq!(parse_bytes("12MB"), Some(12 << 20));
        assert_eq!(parse_bytes("lots"), None);
        assert_eq!(parse_bytes("3X"), None);
        assert_eq!(gib(246.0 * (1u64 << 30) as f64), "246 GiB");
        assert_eq!(gib(0.25 * (1u64 << 30) as f64), "0.25 GiB");
        assert_eq!(gib(3.0e5 * (1u64 << 30) as f64), "3.000e5 GiB");
        // A plan below 1 GiB never reads the machine's memory; the machine's memory, where it
        // can be read, is at least that.
        assert_eq!(check_plan(1.0e6), Ok(()));
        if let Some(m) = physical_memory() {
            assert!(m >= 1 << 28, "{m}");
            assert!(
                check_plan(m as f64 * 2.0).is_err() || std::env::var_os(MEMORY_LIMIT_ENV).is_some()
            );
        }
    }

    #[test]
    fn a_given_limit_comes_first_and_is_restored() {
        let limit = with_memory_limit(64 << 30, memory_limit);
        assert_eq!(limit.bytes, 64 << 30);
        assert_eq!(limit.source, LimitSource::Given);
        assert!(
            limit
                .to_string()
                .contains("64.0 GiB given to this computation")
        );
        with_memory_limit(1 << 20, || {
            // A given limit is read even below 1 GiB.
            assert_eq!(
                check_plan(4.0 * (1 << 20) as f64).unwrap_err().bytes,
                1 << 20
            );
            assert!(check_plan(1000.0).is_ok());
            let inner = with_memory_limit(2 << 20, memory_limit);
            assert_eq!(inner.bytes, 2 << 20);
            assert_eq!(memory_limit().bytes, 1 << 20);
        });
        assert_ne!(memory_limit().source, LimitSource::Given);
        let caught = std::panic::catch_unwind(|| with_memory_limit(5, || panic!("inside")));
        assert!(caught.is_err());
        assert_ne!(memory_limit().source, LimitSource::Given);
        // Fail-safe: every source gives a limit, the default where nothing can be read.
        let default = MemoryLimit {
            bytes: DEFAULT_LIMIT,
            source: LimitSource::Default,
        };
        assert!(default.to_string().contains("could not be read"));
    }

    #[test]
    fn impossible_allocations_are_errors() {
        let e = filled(usize::MAX / 2, 0.0f32).unwrap_err();
        assert_eq!(e.element_bytes, 4);
        assert!(
            e.to_string()
                .starts_with("Failed to allocate memory for image")
        );
        assert!(filled(1usize << 62, 0u8).is_err());
    }
}
