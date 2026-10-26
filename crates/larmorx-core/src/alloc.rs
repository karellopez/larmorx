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

/// Plans up to this size never need the machine's memory (every machine larmorx supports has
/// it): the machine is queried only for a larger plan, with no limit given or set (2026-10-26).
pub const QUERY_ABOVE: u64 = 4 << 30;

/// How long the Windows query of the machine's memory may take before the fail-safe default.
pub const QUERY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

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
/// Windows: the computer system's `TotalPhysicalMemory`, from PowerShell within
/// [`QUERY_TIMEOUT`]), or `None` where it cannot be read. Read once per process.
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
fn read_physical_memory() -> Option<u64> {
    // The computer system's `TotalPhysicalMemory` from PowerShell's CIM query, with a time
    // limit: the process took over 5 s on the windows-arm64 runner (CI, 2026-10-26). No unsafe
    // code and no dependency (CLAUDE.md rule 5); where it fails or times out, `None`, and the
    // limit is the fail-safe default. It runs at most once per process, and only for a plan
    // above `QUERY_ABOVE`.
    use std::io::Read;
    use std::process::{Command, Stdio};
    let mut child = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "(Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = std::time::Instant::now() + QUERY_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let mut out = String::new();
    child.stdout.take()?.read_to_string(&mut out).ok()?;
    out.trim().parse().ok().filter(|&bytes: &u64| bytes > 0)
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
/// Up to [`QUERY_ABOVE`], with no limit given or set, the machine's memory is not read.
pub fn check_plan(needed: f64) -> Result<(), MemoryLimit> {
    plan_against(needed, physical_memory)
}

/// [`check_plan`] with the machine's memory from `physical`, called only where the plan needs
/// it (a plan above [`QUERY_ABOVE`] with no limit given or set).
fn plan_against(needed: f64, physical: impl FnOnce() -> Option<u64>) -> Result<(), MemoryLimit> {
    let within = |limit: MemoryLimit| {
        if needed > limit.bytes as f64 {
            Err(limit)
        } else {
            Ok(())
        }
    };
    if let Some(bytes) = GIVEN.with(Cell::get) {
        return within(MemoryLimit {
            bytes,
            source: LimitSource::Given,
        });
    }
    if let Some(bytes) = std::env::var(MEMORY_LIMIT_ENV)
        .ok()
        .as_deref()
        .and_then(parse_bytes)
    {
        return within(MemoryLimit {
            bytes,
            source: LimitSource::Environment,
        });
    }
    if needed <= QUERY_ABOVE as f64 {
        return Ok(());
    }
    within(match physical() {
        Some(bytes) => MemoryLimit {
            bytes,
            source: LimitSource::Physical,
        },
        None => MemoryLimit {
            bytes: DEFAULT_LIMIT,
            source: LimitSource::Default,
        },
    })
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
    fn a_small_plan_never_queries_the_machine() {
        if std::env::var_os(MEMORY_LIMIT_ENV).is_some() {
            return; // a limit set for the whole run comes first
        }
        let never = || -> Option<u64> { panic!("the machine's memory was queried") };
        assert!(plan_against(1.0e6, never).is_ok());
        assert!(plan_against(QUERY_ABOVE as f64, never).is_ok());
        // Above it, the machine's memory, or the fail-safe default where it cannot be read.
        let eight = || Some(8u64 << 30);
        assert!(plan_against(6.0 * (1u64 << 30) as f64, eight).is_ok());
        let e = plan_against(16.0 * (1u64 << 30) as f64, eight).unwrap_err();
        assert_eq!((e.bytes, e.source), (8 << 30, LimitSource::Physical));
        assert!(plan_against(512.0 * (1u64 << 30) as f64, || None).is_ok());
        let e = plan_against(2.0 * DEFAULT_LIMIT as f64, || None).unwrap_err();
        assert_eq!(e.source, LimitSource::Default);
        // A given limit is never the machine's.
        with_memory_limit(1 << 20, || {
            assert!(plan_against(4.0 * (1 << 20) as f64, never).is_err());
        });
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
