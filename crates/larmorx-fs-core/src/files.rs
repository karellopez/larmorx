// SPDX-License-Identifier: Apache-2.0
//! File names as bytes, and the file helpers of spec §10 that need the file system.

use std::path::PathBuf;

/// A path from a C string's bytes (Unix: as they are; elsewhere: its UTF-8 reading, lossily).
pub fn path_of(name: &[u8]) -> PathBuf {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        PathBuf::from(std::ffi::OsStr::from_bytes(name))
    }
    #[cfg(not(unix))]
    {
        PathBuf::from(String::from_utf8_lossy(name).into_owned())
    }
}

/// `FileExists`: whether the file opens for reading (a directory does, on Linux; a file without
/// read permission does not). FreeSurfer's `errno` is left unchanged.
pub fn file_exists(name: &[u8]) -> bool {
    !name.is_empty() && std::fs::File::open(path_of(name)).is_ok()
}
