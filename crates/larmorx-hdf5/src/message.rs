// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! Decoders of the object header messages a reader of groups and datasets needs: dataspace,
//! datatype, fill value, layout, filter pipeline, link, link info, group info, symbol table,
//! and the shared-message wrapper (committed datatypes).
//!
//! Follows the `decode` callbacks of HDF5 1.12.1: `src/H5Osdspace.c`, `src/H5Odtype.c`,
//! `src/H5Ofill.c`, `src/H5Olayout.c`, `src/H5Opline.c`, `src/H5Olink.c`, `src/H5Olinfo.c`,
//! `src/H5Oginfo.c`, `src/H5Ostab.c`, `src/H5Oshared.c`.

use crate::datatype::Datatype;
use crate::error::{Result, corrupt, unsupported};
use crate::source::{Cursor, UNDEF};

/// A dataspace (`H5S_extent_t`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Dataspace {
    Scalar,
    Null,
    Simple {
        dims: Vec<u64>,
        max_dims: Option<Vec<u64>>,
    },
}

impl Dataspace {
    /// The number of elements (1 for a scalar, 0 for a null dataspace).
    pub fn npoints(&self) -> u64 {
        match self {
            Dataspace::Scalar => 1,
            Dataspace::Null => 0,
            Dataspace::Simple { dims, .. } => dims.iter().product(),
        }
    }

    /// The rank (`H5Sget_simple_extent_ndims`): 0 for scalar and null dataspaces.
    pub fn rank(&self) -> usize {
        match self {
            Dataspace::Simple { dims, .. } => dims.len(),
            _ => 0,
        }
    }

    pub fn dims(&self) -> &[u64] {
        match self {
            Dataspace::Simple { dims, .. } => dims,
            _ => &[],
        }
    }
}

pub(crate) fn dataspace(c: &mut Cursor) -> Result<Dataspace> {
    let version = c.u8()?;
    if !(1..=2).contains(&version) {
        return Err(corrupt("wrong version number in dataspace message"));
    }
    let rank = usize::from(c.u8()?);
    if rank > 32 {
        return Err(corrupt("simple dataspace dimensionality is too large"));
    }
    let flags = c.u8()?;
    let class = if version >= 2 {
        let t = c.u8()?;
        if t != 1 && rank > 0 {
            return Err(corrupt("invalid rank for scalar or NULL dataspace"));
        }
        t
    } else {
        c.skip(1)?;
        if rank > 0 { 1 } else { 0 }
    };
    if version == 1 {
        c.skip(4)?;
    }
    let mut dims = Vec::with_capacity(rank);
    for _ in 0..rank {
        dims.push(c.length()?);
    }
    let max_dims = if rank > 0 && flags & 0x01 != 0 {
        let mut m = Vec::with_capacity(rank);
        for _ in 0..rank {
            m.push(c.length()?);
        }
        Some(m)
    } else {
        None
    };
    Ok(match class {
        0 => Dataspace::Scalar,
        2 => Dataspace::Null,
        1 => Dataspace::Simple { dims, max_dims },
        _ => return Err(corrupt("unknown dataspace type")),
    })
}

/// A fill value (`H5O_fill_t`): the value, if one is defined.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FillValue {
    pub version: u8,
    pub alloc_time: u8,
    pub fill_time: u8,
    pub defined: bool,
    pub value: Option<Vec<u8>>,
}

pub(crate) fn fill_new(c: &mut Cursor) -> Result<FillValue> {
    let version = c.u8()?;
    if !(1..=3).contains(&version) {
        return Err(corrupt("bad version number for fill value message"));
    }
    let mut f = FillValue {
        version,
        ..Default::default()
    };
    if version < 3 {
        f.alloc_time = c.u8()?;
        f.fill_time = c.u8()?;
        f.defined = c.u8()? != 0;
        if f.defined {
            let size = c.u32()? as i32;
            if size > 0 {
                f.value = Some(c.bytes(size as usize)?.to_vec());
            }
        }
    } else {
        let flags = c.u8()?;
        if flags & !0x3f != 0 {
            return Err(corrupt("unknown flag for fill value message"));
        }
        f.alloc_time = flags & 0x03;
        f.fill_time = (flags >> 2) & 0x03;
        if flags & 0x10 != 0 {
            f.defined = false;
        } else if flags & 0x20 != 0 {
            let size = c.u32()? as usize;
            f.value = Some(c.bytes(size)?.to_vec());
            f.defined = true;
        } else {
            f.defined = true;
        }
    }
    Ok(f)
}

/// The old fill-value message (type 4): a size and the value.
pub(crate) fn fill_old(c: &mut Cursor) -> Result<FillValue> {
    let size = c.u32()? as usize;
    Ok(FillValue {
        version: 0,
        alloc_time: 0,
        fill_time: 0,
        defined: size > 0,
        value: if size > 0 {
            Some(c.bytes(size)?.to_vec())
        } else {
            None
        },
    })
}

/// How a chunked dataset indexes its chunks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChunkIndex {
    /// Layout versions 1 to 3: a version-1 B-tree.
    BTreeV1,
    /// One chunk the size of the dataset; with filters, its stored size and filter mask.
    Single {
        filtered: Option<(u64, u32)>,
    },
    /// Chunks at fixed places, unfiltered (`H5D_CHUNK_IDX_NONE`).
    Implicit,
    FixedArray {
        page_bits: u8,
    },
    ExtensibleArray {
        max_nelmts_bits: u8,
        idx_blk_elmts: u8,
        sup_blk_min_data_ptrs: u8,
        data_blk_min_elmts: u8,
        max_dblk_page_nelmts_bits: u8,
    },
    BTreeV2 {
        node_size: u32,
        split_percent: u8,
        merge_percent: u8,
    },
}

/// Where and how a dataset's values are stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Layout {
    Compact {
        version: u8,
        data: Vec<u8>,
    },
    Contiguous {
        version: u8,
        addr: u64,
        /// Stored size (layout version 3 and later).
        size: Option<u64>,
    },
    Chunked {
        version: u8,
        flags: u8,
        /// Chunk dimensions; the last one is the element size (in bytes).
        dims: Vec<u64>,
        index: ChunkIndex,
        addr: u64,
    },
    Virtual,
}

pub(crate) fn layout(c: &mut Cursor) -> Result<Layout> {
    let version = c.u8()?;
    if !(1..=4).contains(&version) {
        return Err(corrupt("bad version number for layout message"));
    }
    if version < 3 {
        let ndims = usize::from(c.u8()?);
        if ndims == 0 || ndims > 33 {
            return Err(corrupt("dimensionality is out of range"));
        }
        let class = c.u8()?;
        c.skip(5)?;
        let addr = if class == 1 || class == 2 {
            c.addr()?
        } else {
            UNDEF
        };
        let mut dims = Vec::with_capacity(ndims);
        for _ in 0..ndims {
            dims.push(u64::from(c.u32()?));
        }
        return Ok(match class {
            0 => {
                let size = c.u32()? as usize;
                Layout::Compact {
                    version,
                    data: c.bytes(size)?.to_vec(),
                }
            }
            1 => Layout::Contiguous {
                version,
                addr,
                size: None,
            },
            2 => Layout::Chunked {
                version,
                flags: 0,
                dims,
                index: ChunkIndex::BTreeV1,
                addr,
            },
            _ => return Err(corrupt("invalid layout class")),
        });
    }
    let class = c.u8()?;
    Ok(match class {
        0 => {
            let size = usize::from(c.u16()?);
            Layout::Compact {
                version,
                data: c.bytes(size)?.to_vec(),
            }
        }
        1 => Layout::Contiguous {
            version,
            addr: c.addr()?,
            size: Some(c.length()?),
        },
        2 if version < 4 => {
            let ndims = usize::from(c.u8()?);
            if ndims > 33 {
                return Err(corrupt("dimensionality is too large"));
            }
            let addr = c.addr()?;
            let mut dims = Vec::with_capacity(ndims);
            for _ in 0..ndims {
                let d = u64::from(c.u32()?);
                if d == 0 {
                    return Err(corrupt("chunk dimension must be positive"));
                }
                dims.push(d);
            }
            Layout::Chunked {
                version,
                flags: 0,
                dims,
                index: ChunkIndex::BTreeV1,
                addr,
            }
        }
        2 => {
            let flags = c.u8()?;
            if flags & !0x03 != 0 {
                return Err(corrupt("bad flag value for message"));
            }
            let ndims = usize::from(c.u8()?);
            if ndims > 33 {
                return Err(corrupt("dimensionality is too large"));
            }
            let enc = usize::from(c.u8()?);
            if enc == 0 || enc > 8 {
                return Err(corrupt("encoded chunk dimension size is too large"));
            }
            let mut dims = Vec::with_capacity(ndims);
            for _ in 0..ndims {
                dims.push(c.uvar(enc)?);
            }
            // H5D_chunk_index_t: 0 v1 B-tree, 1 single, 2 none (implicit), 3 fixed array,
            // 4 extensible array, 5 v2 B-tree.
            let index = match c.u8()? {
                0 => {
                    return Err(corrupt(
                        "v1 B-tree index type should never be in a v4 layout message",
                    ));
                }
                2 => ChunkIndex::Implicit,
                1 => {
                    let filtered = if flags & 0x02 != 0 {
                        Some((c.length()?, c.u32()?))
                    } else {
                        None
                    };
                    ChunkIndex::Single { filtered }
                }
                3 => {
                    let page_bits = c.u8()?;
                    if page_bits == 0 {
                        return Err(corrupt("invalid fixed array creation parameter"));
                    }
                    ChunkIndex::FixedArray { page_bits }
                }
                4 => {
                    let p = [c.u8()?, c.u8()?, c.u8()?, c.u8()?, c.u8()?];
                    if p.contains(&0) {
                        return Err(corrupt("invalid extensible array creation parameter"));
                    }
                    ChunkIndex::ExtensibleArray {
                        max_nelmts_bits: p[0],
                        idx_blk_elmts: p[1],
                        sup_blk_min_data_ptrs: p[2],
                        data_blk_min_elmts: p[3],
                        max_dblk_page_nelmts_bits: p[4],
                    }
                }
                5 => ChunkIndex::BTreeV2 {
                    node_size: c.u32()?,
                    split_percent: c.u8()?,
                    merge_percent: c.u8()?,
                },
                _ => return Err(corrupt("unknown chunk index type")),
            };
            Layout::Chunked {
                version,
                flags,
                dims,
                index,
                addr: c.addr()?,
            }
        }
        3 => {
            if version < 4 {
                return Err(corrupt("invalid layout version with virtual layout"));
            }
            Layout::Virtual
        }
        _ => return Err(corrupt("Invalid layout class")),
    })
}

/// One filter of a pipeline (`H5Z_filter_info_t`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filter {
    pub id: u16,
    pub flags: u16,
    pub name: Option<String>,
    pub cd_values: Vec<u32>,
}

impl Filter {
    /// `H5Z_FLAG_OPTIONAL`: the filter may fail on a chunk, which is then stored unfiltered.
    pub fn optional(&self) -> bool {
        self.flags & 0x0001 != 0
    }
}

pub(crate) fn pipeline(c: &mut Cursor) -> Result<Vec<Filter>> {
    let version = c.u8()?;
    if !(1..=2).contains(&version) {
        return Err(corrupt("bad version number for filter pipeline message"));
    }
    let n = usize::from(c.u8()?);
    if n > 32 {
        return Err(corrupt("filter pipeline message has too many filters"));
    }
    if version == 1 {
        c.skip(6)?;
    }
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let id = c.u16()?;
        let name_len = if version > 1 && id < 256 {
            0
        } else {
            let l = usize::from(c.u16()?);
            if version == 1 && l % 8 != 0 {
                return Err(corrupt("filter name length is not a multiple of eight"));
            }
            l
        };
        let flags = c.u16()?;
        let nvalues = usize::from(c.u16()?);
        let name = if name_len > 0 {
            let raw = c.bytes(name_len)?;
            let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
            Some(String::from_utf8_lossy(&raw[..end]).into_owned())
        } else {
            None
        };
        let mut cd_values = Vec::with_capacity(nvalues);
        for _ in 0..nvalues {
            cd_values.push(c.u32()?);
        }
        if version == 1 && nvalues % 2 == 1 {
            c.skip(4)?;
        }
        out.push(Filter {
            id,
            flags,
            name,
            cd_values,
        });
    }
    Ok(out)
}

/// Where a link points.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkTarget {
    Hard(u64),
    Soft(String),
    /// External (type 64) and user-defined links: the type and its data.
    UserDefined {
        link_type: u8,
        data: Vec<u8>,
    },
}

/// A link (`H5O_link_t`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub name: Vec<u8>,
    pub corder: Option<i64>,
    pub utf8: bool,
    pub target: LinkTarget,
}

pub(crate) fn link(c: &mut Cursor) -> Result<Link> {
    if c.u8()? != 1 {
        return Err(corrupt("bad version number for message"));
    }
    let flags = c.u8()?;
    if flags & !0x1f != 0 {
        return Err(corrupt("bad flag value for message"));
    }
    let link_type = if flags & 0x08 != 0 {
        let t = c.u8()?;
        if t != 0 && t != 1 && t < 64 {
            return Err(corrupt("bad link type"));
        }
        t
    } else {
        0
    };
    let corder = if flags & 0x04 != 0 {
        Some(c.u64()? as i64)
    } else {
        None
    };
    let utf8 = if flags & 0x10 != 0 {
        let cset = c.u8()?;
        if cset > 1 {
            return Err(corrupt("bad cset type"));
        }
        cset == 1
    } else {
        false
    };
    let len = c.uvar(1 << (flags & 0x03))? as usize;
    if len == 0 {
        return Err(corrupt("invalid name length"));
    }
    let name = c.bytes(len)?.to_vec();
    let target = match link_type {
        0 => LinkTarget::Hard(c.addr()?),
        1 => {
            let l = usize::from(c.u16()?);
            if l == 0 {
                return Err(corrupt("invalid link length"));
            }
            LinkTarget::Soft(String::from_utf8_lossy(c.bytes(l)?).into_owned())
        }
        t => {
            let l = usize::from(c.u16()?);
            LinkTarget::UserDefined {
                link_type: t,
                data: c.bytes(l)?.to_vec(),
            }
        }
    };
    Ok(Link {
        name,
        corder,
        utf8,
        target,
    })
}

/// Link info (`H5O_linfo_t`): where a new-style group's dense storage is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkInfo {
    pub track_corder: bool,
    pub index_corder: bool,
    pub max_corder: i64,
    pub fheap_addr: u64,
    pub name_bt2_addr: u64,
    pub corder_bt2_addr: u64,
}

pub(crate) fn link_info(c: &mut Cursor) -> Result<LinkInfo> {
    if c.u8()? != 0 {
        return Err(corrupt("bad version number for message"));
    }
    let flags = c.u8()?;
    if flags & !0x03 != 0 {
        return Err(corrupt("bad flag value for message"));
    }
    let track_corder = flags & 0x01 != 0;
    let index_corder = flags & 0x02 != 0;
    let max_corder = if track_corder { c.u64()? as i64 } else { 0 };
    let fheap_addr = c.addr()?;
    let name_bt2_addr = c.addr()?;
    let corder_bt2_addr = if index_corder { c.addr()? } else { UNDEF };
    Ok(LinkInfo {
        track_corder,
        index_corder,
        max_corder,
        fheap_addr,
        name_bt2_addr,
        corder_bt2_addr,
    })
}

/// Group info (`H5O_ginfo_t`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupInfo {
    pub max_compact: u16,
    pub min_dense: u16,
    pub est_num_entries: u16,
    pub est_name_len: u16,
    pub store_phase_change: bool,
    pub store_est_entry_info: bool,
}

/// Decodes a group info message.
#[allow(dead_code)]
pub(crate) fn group_info(c: &mut Cursor) -> Result<GroupInfo> {
    if c.u8()? != 0 {
        return Err(corrupt("bad version number for message"));
    }
    let flags = c.u8()?;
    if flags & !0x03 != 0 {
        return Err(corrupt("bad flag value for message"));
    }
    let store_phase_change = flags & 0x01 != 0;
    let store_est_entry_info = flags & 0x02 != 0;
    let (max_compact, min_dense) = if store_phase_change {
        (c.u16()?, c.u16()?)
    } else {
        (8, 6)
    };
    let (est_num_entries, est_name_len) = if store_est_entry_info {
        (c.u16()?, c.u16()?)
    } else {
        (4, 8)
    };
    Ok(GroupInfo {
        max_compact,
        min_dense,
        est_num_entries,
        est_name_len,
        store_phase_change,
        store_est_entry_info,
    })
}

/// Symbol table (an old-style group's B-tree and local heap).
pub(crate) fn symbol_table(c: &mut Cursor) -> Result<(u64, u64)> {
    Ok((c.addr()?, c.addr()?))
}

/// A shared message (`H5O__shared_decode`): the object header that holds the real message.
pub(crate) fn shared_location(c: &mut Cursor) -> Result<u64> {
    let version = c.u8()?;
    if !(1..=3).contains(&version) {
        return Err(corrupt("bad version number for shared object message"));
    }
    let kind = if version >= 2 {
        c.u8()?
    } else {
        c.skip(1)?;
        2
    };
    if version == 1 {
        c.skip(6)?;
        c.length()?; // the local heap offset of a symbol table entry
        return c.addr();
    }
    if kind == 1 && version >= 3 {
        return Err(unsupported(
            "messages shared in the file's shared-message heap (SOHM)",
        ));
    }
    c.addr()
}

/// Decodes a datatype message, recursively (`H5O__dtype_decode_helper`).
pub(crate) fn datatype(c: &mut Cursor) -> Result<Datatype> {
    crate::datatype::decode(c)
}
