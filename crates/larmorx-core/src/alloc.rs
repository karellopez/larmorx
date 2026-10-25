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

use std::sync::OnceLock;

/// The memory larmorx plans a computation for: `LARMORX_MEMORY_LIMIT` (bytes, or with a binary
/// suffix: `512M`, `64G`, `2T`) when set, else the machine's physical memory.
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

/// Where a [`MemoryLimit`] comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimitSource {
    /// `LARMORX_MEMORY_LIMIT`.
    Environment,
    /// The machine's physical memory.
    Physical,
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
/// Windows: the computer system's `TotalPhysicalMemory`), or `None` where it cannot be read.
/// Read once.
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
    let out = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "(Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory",
        ])
        .output()
        .ok()?;
    String::from_utf8(out.stdout).ok()?.trim().parse().ok()
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

/// The memory larmorx may plan for ([`MEMORY_LIMIT_ENV`], else the physical memory), or `None`
/// where neither is known (no limit).
pub fn memory_limit() -> Option<MemoryLimit> {
    if let Some(bytes) = std::env::var(MEMORY_LIMIT_ENV)
        .ok()
        .as_deref()
        .and_then(parse_bytes)
    {
        return Some(MemoryLimit {
            bytes,
            source: LimitSource::Environment,
        });
    }
    physical_memory().map(|bytes| MemoryLimit {
        bytes,
        source: LimitSource::Physical,
    })
}

/// `Err(limit)` when a computation that needs `needed` bytes exceeds the [`memory_limit`].
/// Below 1 GiB the machine's memory is not even read (every machine larmorx runs on has it).
pub fn check_plan(needed: f64) -> Result<(), MemoryLimit> {
    let env = std::env::var_os(MEMORY_LIMIT_ENV).is_some();
    if !env && needed < (1u64 << 30) as f64 {
        return Ok(());
    }
    match memory_limit() {
        Some(limit) if needed > limit.bytes as f64 => Err(limit),
        _ => Ok(()),
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
