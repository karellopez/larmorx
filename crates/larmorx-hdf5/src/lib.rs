// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! HDF5 in pure Rust, for the files larmorx reads and writes (CLAUDE.md rule 5: no HDF5 C
//! library).
//!
//! - **Reading** ([`File`]): superblocks 0 to 3; object headers 1 and 2 with continuation
//!   chunks; old-style (symbol table), compact and dense groups (fractal heaps, filtered or
//!   not); soft links; compact, contiguous and chunked datasets (every chunk index: version-1
//!   B-tree, single chunk, implicit, fixed array, extensible array, version-2 B-tree); the
//!   deflate, shuffle, Fletcher-32, n-bit and scale-offset filters; fill values; committed
//!   datatypes and messages shared in the shared-message heaps (SOHM); floating-point data of
//!   any layout converted as HDF5 1.12.1's `H5Dread` converts it on x86-64; fixed- and
//!   variable-length strings. Not read: data stored elsewhere (external raw files, external
//!   links, virtual datasets).
//! - **Writing** ([`write`]): the subset of the HDF5 library ITK's transform writer uses
//!   (libver bounds 1.8: superblock 2, version-2 object headers, compact groups, contiguous
//!   and chunked deflated datasets, variable-length strings), laid out byte for byte as
//!   HDF5 1.12.1 lays it out.
//!
//! The format follows The HDF Group's file format specification, version 3; behaviour and
//! layout follow the HDF5 1.12.1 source that ITK v5.4.5 bundles (`PROVENANCE.md`).
#![forbid(unsafe_code)]

pub mod btree;
pub mod checksum;
pub mod dataset;
pub mod datatype;
pub mod error;
pub mod file;
pub mod filters;
pub mod group;
pub mod heap;
pub mod message;
pub mod object;
pub(crate) mod source;
pub mod superblock;
pub mod write;

pub use dataset::{Chunk, DatasetInfo};
pub use datatype::{Datatype, FloatType};
pub use error::{Error, Result};
pub use file::{File, ObjectKind};
pub use group::Storage;
pub use message::{Dataspace, Layout, Link, LinkTarget};
pub use source::UNDEF;
