// SPDX-License-Identifier: Apache-2.0
//! Errors of the HDF5 reader and writer.

use std::path::PathBuf;

/// An HDF5 file cannot be read or written.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The operating system refused a read or a write.
    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// No HDF5 signature at offset 0 or at any power of two from 512 on, as `H5Fis_hdf5`
    /// looks for it.
    #[error("not an HDF5 file (no HDF5 signature)")]
    NotHdf5,
    /// The file is HDF5, but a structure in it is malformed or truncated.
    #[error("corrupt HDF5 file: {0}")]
    Corrupt(String),
    /// A structure's stored checksum does not match its contents.
    #[error("corrupt HDF5 file: incorrect metadata checksum ({0})")]
    Checksum(String),
    /// A valid HDF5 feature this reader does not implement.
    #[error("unsupported HDF5 feature: {0}")]
    Unsupported(String),
    /// A path names no object (a link is missing along it).
    #[error("{0}: no such object in the file")]
    NotFound(String),
    /// An object exists but is not of the kind asked for (a group where a dataset is needed).
    #[error("{0}")]
    WrongKind(String),
    /// A dataset cannot be read as asked (its datatype or dataspace does not convert).
    #[error("{0}")]
    Conversion(String),
    /// A filter of a dataset's pipeline failed (a corrupt deflate stream, a Fletcher-32
    /// mismatch) or is not available.
    #[error("{0}")]
    Filter(String),
    /// A value cannot be written in the layout asked for.
    #[error("{0}")]
    Write(String),
    /// The thread pool could not be created.
    #[error("{0}")]
    Threads(String),
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn corrupt(message: impl Into<String>) -> Error {
    Error::Corrupt(message.into())
}

pub(crate) fn unsupported(message: impl Into<String>) -> Error {
    Error::Unsupported(message.into())
}
