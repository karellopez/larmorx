// SPDX-License-Identifier: Apache-2.0
//! Temporary files (spec §18): the base folder from the environment, writable folders, new
//! temporary files and folders `tmp.XXXXXX` (six letters or digits, unpredictable).

use std::fs::{self, OpenOptions};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::files::path_of;
use crate::process::{Process, Stop};

/// The variables looked at for the base folder, in order.
pub const TEMP_VARIABLES: [&str; 5] = ["FS_TMPDIR", "TMPDIR", "TMP", "TEMP", "TEMPDIR"];

const CHARS: &[u8; 62] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

/// Six unpredictable letters or digits (the clock, the process and a counter, mixed).
fn six(seed: &mut u64) -> [u8; 6] {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    *seed = seed
        .wrapping_add(nanos)
        .wrapping_add(u64::from(std::process::id()) << 32)
        .wrapping_add(0x9e37_79b9_7f4a_7c15);
    // splitmix64's finaliser
    let mut z = *seed;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^= z >> 31;
    let mut out = [0u8; 6];
    for o in &mut out {
        *o = CHARS[(z % 62) as usize];
        z /= 62;
    }
    out
}

/// `<dir>/tmp.XXXXXX<suffix>` with fresh characters.
fn candidate(dir: &[u8], suffix: &[u8], seed: &mut u64) -> Vec<u8> {
    let mut p = dir.to_vec();
    p.extend_from_slice(b"/tmp.");
    p.extend_from_slice(&six(seed));
    p.extend_from_slice(suffix);
    p
}

/// Makes a new empty file (`mkstemps`), retrying names that exist; its path, or `None`.
fn new_file(dir: &[u8], suffix: &[u8]) -> Option<Vec<u8>> {
    let mut seed = 0;
    for _ in 0..100 {
        let p = candidate(dir, suffix, &mut seed);
        let mut o = OpenOptions::new();
        o.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            o.mode(0o600);
        }
        match o.open(path_of(&p)) {
            Ok(_) => return Some(p),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return None,
        }
    }
    None
}

/// Makes a new folder (`mkdtemp`); its path, or `None`.
fn new_dir(dir: &[u8]) -> Option<Vec<u8>> {
    let mut seed = 0;
    for _ in 0..100 {
        let p = candidate(dir, b"", &mut seed);
        let mut b = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            b.mode(0o700);
        }
        match b.create(path_of(&p)) {
            Ok(()) => return Some(p),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return None,
        }
    }
    None
}

/// `directoryExists`: whether `path` is an existing folder.
pub fn directory_exists(path: &[u8]) -> bool {
    !path.is_empty() && Path::is_dir(&path_of(path))
}

/// `directoryIsWritable`: whether `path` is an existing folder in which a new file can be made
/// (it is made, then removed).
pub fn directory_is_writable(path: &[u8]) -> bool {
    if !directory_exists(path) {
        return false;
    }
    match new_file(path, b"") {
        Some(p) => {
            let _ = fs::remove_file(path_of(&p));
            true
        }
        None => false,
    }
}

impl Process {
    /// The base folder of temporary files (`getBaseTempDir`): the first of [`TEMP_VARIABLES`]
    /// that is set, not empty and writable; else `TEMPDIR`'s value, or `/tmp` when that is empty
    /// or unset, which must be writable: else `error: Can not write to temporary directory
    /// <dir>` and the end of the program with status 1.
    pub fn base_temp_dir(&mut self) -> Result<Vec<u8>, Stop> {
        for v in TEMP_VARIABLES {
            if let Some(d) = self.env.non_empty(v)
                && directory_is_writable(d)
            {
                return Ok(d.to_vec());
            }
        }
        let dir = self
            .env
            .non_empty("TEMPDIR")
            .map_or_else(|| b"/tmp".to_vec(), <[u8]>::to_vec);
        if !directory_is_writable(&dir) {
            let mut m = b"Can not write to temporary directory ".to_vec();
            m.extend_from_slice(&dir);
            self.fatal(1, &m)?;
        }
        Ok(dir)
    }

    /// A new empty temporary file `<base>/tmp.XXXXXX<suffix>` (`makeTempFile`); failure: `error:
    /// Could not create temporary file in <base>`, status 1.
    pub fn make_temp_file(&mut self, suffix: &[u8]) -> Result<Vec<u8>, Stop> {
        let base = self.base_temp_dir()?;
        match new_file(&base, suffix) {
            Some(p) => Ok(p),
            None => {
                let mut m = b"Could not create temporary file in ".to_vec();
                m.extend_from_slice(&base);
                self.fatal(1, &m)?;
                Ok(Vec::new())
            }
        }
    }

    /// A new temporary folder `<base>/tmp.XXXXXX` (`makeTempDir`); failure: `error: Could not
    /// create temporary directory in <base>`, status 1.
    pub fn make_temp_dir(&mut self) -> Result<Vec<u8>, Stop> {
        let base = self.base_temp_dir()?;
        match new_dir(&base) {
            Some(p) => Ok(p),
            None => {
                let mut m = b"Could not create temporary directory in ".to_vec();
                m.extend_from_slice(&base);
                self.fatal(1, &m)?;
                Ok(Vec::new())
            }
        }
    }
}
