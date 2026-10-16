// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! An HDF5 file opened for reading: its superblock, objects found by path, groups' links and
//! datasets' values.

use std::path::Path;

use crate::dataset::{self, DatasetInfo};
use crate::datatype::{self, Datatype, StrPad};
use crate::error::{Error, Result, corrupt, unsupported};
use crate::group::{self, Storage};
use crate::heap;
use crate::message::{Dataspace, Layout, Link, LinkTarget};
use crate::object::{self, ObjectHeader};
use crate::source::{Cursor, Source};
use crate::superblock::{self, Superblock};

/// What an object is (`H5O_type_t`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectKind {
    Group,
    Dataset,
    NamedDatatype,
    Unknown,
}

/// An HDF5 file, read on demand.
pub struct File {
    src: Source,
    sb: Superblock,
}

/// The maximum number of soft links followed along a path (`H5L_NUM_LINKS`).
const MAX_SOFT_LINKS: usize = 16;

/// `H5D_TEMP_BUF_SIZE`: the size of `H5Dread`'s type conversion buffer.
const TEMP_BUF_SIZE: usize = 1024 * 1024;

impl File {
    /// Opens `path` (`H5Fopen` with `H5F_ACC_RDONLY`): finds the superblock and checks it.
    pub fn open(path: &Path) -> Result<File> {
        File::from_source(Source::open(path)?)
    }

    /// Reads a file held in memory.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<File> {
        File::from_source(Source::Memory(bytes))
    }

    fn from_source(src: Source) -> Result<File> {
        let at = superblock::locate(&src)?.ok_or(Error::NotHdf5)?;
        let sb = superblock::read(&src, at)?;
        Ok(File { src, sb })
    }

    /// `H5Fis_hdf5`: whether the file has an HDF5 signature where HDF5 looks for one.
    pub fn is_hdf5(path: &Path) -> Result<bool> {
        let src = Source::open(path)?;
        Ok(superblock::locate(&src)?.is_some())
    }

    pub fn superblock(&self) -> &Superblock {
        &self.sb
    }

    /// `len` bytes at file address `addr` (relative to the base address); an address beyond
    /// the end of the file's address space is an error, as HDF5's "addr overflow".
    pub(crate) fn read_at(&self, addr: u64, len: usize) -> Result<Vec<u8>> {
        let end = addr.checked_add(len as u64);
        if end.is_none_or(|e| e > self.sb.eof) {
            return Err(corrupt(format!(
                "addr overflow, addr = {addr}, size = {len}, eoa = {}",
                self.sb.eof
            )));
        }
        self.src.read_at(self.sb.base + addr, len)
    }

    /// Up to `len` bytes at `addr`, fewer at the end of the address space.
    pub(crate) fn read_up_to(&self, addr: u64, len: usize) -> Result<Vec<u8>> {
        let avail = self.sb.eof.saturating_sub(addr);
        self.read_at(addr, len.min(avail as usize))
    }

    /// The object header at `addr`.
    pub fn object_header(&self, addr: u64) -> Result<ObjectHeader> {
        object::read(self, addr)
    }

    /// The root group's object header address.
    pub fn root(&self) -> u64 {
        self.sb.root
    }

    /// What the object at `addr` is (`H5O__obj_class`: group, then dataset, then datatype).
    pub fn kind(&self, addr: u64) -> Result<ObjectKind> {
        let oh = self.object_header(addr)?;
        kind_of(self, &oh)
    }

    /// The links of the group at `addr`, in name order.
    pub fn links(&self, addr: u64) -> Result<Vec<Link>> {
        let oh = self.object_header(addr)?;
        if group::storage(self, &oh)?.is_none() {
            return Err(Error::WrongKind("not a group".into()));
        }
        group::links(self, &oh)
    }

    /// How the group at `addr` stores its links.
    pub fn group_storage(&self, addr: u64) -> Result<Storage> {
        let oh = self.object_header(addr)?;
        group::storage(self, &oh)?.ok_or_else(|| Error::WrongKind("not a group".into()))
    }

    /// Follows `path` (absolute, or relative to the root) to an object's address, through
    /// soft links (`H5G_traverse`). Empty components and `.` are skipped.
    pub fn resolve(&self, path: &str) -> Result<u64> {
        self.resolve_from(self.sb.root, path, 0)
    }

    fn resolve_from(&self, start: u64, path: &str, depth: usize) -> Result<u64> {
        let mut here = if path.starts_with('/') {
            self.sb.root
        } else {
            start
        };
        let comps: Vec<&str> = path
            .split('/')
            .filter(|c| !c.is_empty() && *c != ".")
            .collect();
        for (i, comp) in comps.iter().enumerate() {
            let oh = self.object_header(here)?;
            if group::storage(self, &oh)?.is_none() {
                return Err(Error::NotFound(path.to_string()));
            }
            let links = group::links(self, &oh)?;
            let link = links
                .iter()
                .find(|l| l.name == comp.as_bytes())
                .ok_or_else(|| Error::NotFound(path.to_string()))?;
            here = match &link.target {
                LinkTarget::Hard(addr) => *addr,
                LinkTarget::Soft(value) => {
                    if depth >= MAX_SOFT_LINKS {
                        return Err(corrupt("too many links"));
                    }
                    let _ = i;
                    self.resolve_from(here, value, depth + 1)
                        .map_err(|_| Error::NotFound(path.to_string()))?
                }
                LinkTarget::UserDefined { link_type, .. } => {
                    return Err(unsupported(if *link_type == 64 {
                        "external links".to_string()
                    } else {
                        format!("user-defined links (type {link_type})")
                    }));
                }
            };
        }
        Ok(here)
    }

    /// `H5Lexists` along `path`: whether every link of it exists (the last one may dangle).
    pub fn exists(&self, path: &str) -> Result<bool> {
        let comps: Vec<&str> = path
            .split('/')
            .filter(|c| !c.is_empty() && *c != ".")
            .collect();
        let Some((last, parents)) = comps.split_last() else {
            return Ok(true);
        };
        let parent = parents.join("/");
        let here = match self.resolve(&format!("/{parent}")) {
            Ok(a) => a,
            Err(Error::NotFound(_)) => return Ok(false),
            Err(e) => return Err(e),
        };
        let oh = self.object_header(here)?;
        if group::storage(self, &oh)?.is_none() {
            return Ok(false);
        }
        Ok(group::links(self, &oh)?
            .iter()
            .any(|l| l.name == last.as_bytes()))
    }

    /// A dataset's datatype, dataspace, layout, filters and fill value.
    pub fn dataset(&self, addr: u64) -> Result<DatasetInfo> {
        let oh = self.object_header(addr)?;
        if !dataset::is_dataset(&oh) {
            return Err(Error::WrongKind("not a dataset".into()));
        }
        dataset::info(self, &oh)
    }

    /// Every element of a dataset as stored (row-major, the datatype's size each).
    pub fn read_raw(&self, info: &DatasetInfo, n_threads: usize) -> Result<Vec<u8>> {
        dataset::read_raw(self, info, n_threads)
    }

    /// A floating-point dataset's values as `H5Dread` converts them to x86-64's native
    /// `double` (`as_double`) or `float`, widened to f64. `reproduce` keeps HDF5's
    /// conversion state from one element to the next (HDF-01); the second value says whether
    /// correcting it changed a value. An error for other classes.
    pub fn read_floats(
        &self,
        info: &DatasetInfo,
        as_double: bool,
        n_threads: usize,
        reproduce: bool,
    ) -> Result<(Vec<f64>, bool)> {
        let Datatype::Float(ft) = &info.dtype else {
            return Err(Error::Conversion(
                "no appropriate function for conversion path".into(),
            ));
        };
        let raw = self.read_raw(info, n_threads)?;
        let dst_size = if as_double { 8 } else { 4 };
        let strip = (TEMP_BUF_SIZE / ft.size.max(dst_size)).max(1);
        // H5Dread converts chunk by chunk: a strip never spans two chunks of a 1-D dataset.
        let piece = match (&info.layout, info.space.rank()) {
            (Layout::Chunked { dims, .. }, 1) => dims[0] as usize,
            _ => raw.len() / ft.size.max(1),
        }
        .max(1);
        let mut out = Vec::with_capacity(raw.len() / ft.size.max(1));
        let mut corrected = false;
        for part in raw.chunks(piece * ft.size) {
            let (v, c) = datatype::convert_floats(ft, as_double, part, strip, reproduce)?;
            out.extend(v);
            corrected |= c;
        }
        Ok((out, corrected))
    }

    /// A string dataset's elements, as bytes. Variable-length strings are read from the
    /// global heap (a null one is `None`); fixed-length strings end at their padding.
    pub fn read_strings(&self, info: &DatasetInfo) -> Result<Vec<Option<Vec<u8>>>> {
        let raw = self.read_raw(info, 1)?;
        let elem = info.dtype.size();
        match &info.dtype {
            Datatype::VarLen { string: true, .. } => {
                let (a, s) = (self.sb.sizeof_addr, self.sb.sizeof_size);
                raw.chunks_exact(elem)
                    .map(|e| {
                        let mut c = Cursor::new(e, a, s, "a variable-length string");
                        let len = c.u32()? as usize;
                        let coll = c.addr()?;
                        let idx = c.u32()?;
                        if coll == 0 || coll == crate::source::UNDEF {
                            return Ok(None);
                        }
                        let obj = heap::global_object(self, coll, idx)?;
                        if obj.len() < len {
                            return Err(corrupt("variable-length string shorter than its length"));
                        }
                        Ok(Some(obj[..len].to_vec()))
                    })
                    .collect()
            }
            Datatype::String { pad, .. } => Ok(raw
                .chunks_exact(elem)
                .map(|e| {
                    let end = match pad {
                        StrPad::NullTerm => e.iter().position(|&b| b == 0).unwrap_or(e.len()),
                        StrPad::NullPad => e.iter().rposition(|&b| b != 0).map_or(0, |p| p + 1),
                        StrPad::SpacePad => e.iter().rposition(|&b| b != b' ').map_or(0, |p| p + 1),
                        StrPad::Other(_) => e.len(),
                    };
                    Some(e[..end].to_vec())
                })
                .collect()),
            _ => Err(Error::Conversion(
                "no appropriate function for conversion path".into(),
            )),
        }
    }
}

pub(crate) fn kind_of(file: &File, oh: &ObjectHeader) -> Result<ObjectKind> {
    if group::storage(file, oh)?.is_some() {
        return Ok(ObjectKind::Group);
    }
    if dataset::is_dataset(oh) {
        return Ok(ObjectKind::Dataset);
    }
    if oh.has(object::msg::DATATYPE) {
        return Ok(ObjectKind::NamedDatatype);
    }
    Ok(ObjectKind::Unknown)
}

impl DatasetInfo {
    /// The number of elements.
    pub fn len(&self) -> u64 {
        self.space.npoints()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The rank, as `H5Sget_simple_extent_ndims` gives it.
    pub fn rank(&self) -> usize {
        self.space.rank()
    }

    pub fn is_scalar(&self) -> bool {
        matches!(self.space, Dataspace::Scalar)
    }

    /// The storage layout in a word, with the chunk index and filters of a chunked layout.
    pub fn layout_kind(&self) -> String {
        match &self.layout {
            Layout::Compact { .. } => "compact".into(),
            Layout::Contiguous { .. } => "contiguous".into(),
            Layout::Chunked { dims, index, .. } => format!(
                "chunked {dims:?} {index:?} filters {:?}",
                self.pipeline.iter().map(|f| f.id).collect::<Vec<_>>()
            ),
            Layout::Virtual => "virtual".into(),
        }
    }
}
