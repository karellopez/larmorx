// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! Writing HDF5 files as HDF5 1.12.1 lays them out, for the calls ITK's transform writer makes
//! (`itkHDF5TransformIO.cxx`): a file created with library-version bounds 1.8 (superblock 2,
//! version-2 object headers with times, new-style compact groups), variable-length string
//! datasets, and floating-point datasets either contiguous or chunked with deflate.
//!
//! [`Writer`] replays HDF5's own sequence of allocations and object-header edits, so a file
//! written with the same calls has the same bytes as HDF5's, given the same object time and
//! the same compressed chunks. A group's ninth link would make HDF5 convert it to dense
//! storage (a fractal heap and a version-2 B-tree); the writer refuses that for now.

mod alloc;
mod btree;
mod encode;
mod gheap;
mod ohdr;

use alloc::{Mem, Space};
use btree::{ChunkBTree, ChunkRecord};
use encode::{Native, WLayout, WType, empty_link_info};
use gheap::GlobalHeap;
use ohdr::ObjHeader;

use crate::error::{Error, Result};
use crate::source::UNDEF;

/// `H5O_MSG_FLAG_CONSTANT`.
const CONSTANT: u8 = 0x01;
/// `H5G_CRT_GINFO_MAX_COMPACT`.
const MAX_COMPACT: usize = 8;
/// `H5D_MINHDR_SIZE`.
const DATASET_HEADER: usize = 256;
/// `H5D_CHUNK_CACHE_NBYTES_DEFAULT`: chunks larger than this go straight to the file.
const CHUNK_CACHE: usize = 1024 * 1024;
/// The size of a version-2 superblock with 8-byte addresses.
const SUPERBLOCK: u64 = 48;

/// A group of the file being written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroupId(usize);

/// Floating-point values to store, in the precision to store them in.
#[derive(Clone, Copy, Debug)]
pub enum Floats<'a> {
    F32(&'a [f32]),
    F64(&'a [f64]),
}

impl Floats<'_> {
    fn len(&self) -> usize {
        match self {
            Floats::F32(v) => v.len(),
            Floats::F64(v) => v.len(),
        }
    }

    fn wtype(&self) -> WType {
        match self {
            Floats::F32(_) => WType::F32,
            Floats::F64(_) => WType::F64,
        }
    }

    /// Elements `start..end` as little-endian bytes.
    fn bytes(&self, start: usize, end: usize) -> Vec<u8> {
        match self {
            Floats::F32(v) => v[start..end].iter().flat_map(|x| x.to_le_bytes()).collect(),
            Floats::F64(v) => v[start..end].iter().flat_map(|x| x.to_le_bytes()).collect(),
        }
    }
}

/// How a dataset's values are stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Storage {
    /// One contiguous block (`H5D_CONTIGUOUS`, the default).
    Contiguous,
    /// Chunks of `chunk` elements compressed by deflate at `level` (`H5Pset_chunk`,
    /// `H5Pset_deflate`).
    Chunked { chunk: u64, level: u32 },
}

/// Compresses one chunk as `compress2` does at a level (`H5Z_filter_deflate`).
pub type Deflate = fn(&[u8], u32) -> Vec<u8>;

/// The deflate the writer uses unless told otherwise: zlib-rs through flate2.
pub fn zlib_rs_deflate(data: &[u8], level: u32) -> Vec<u8> {
    use std::io::Write;
    let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::new(level));
    e.write_all(data).expect("compression into memory");
    e.finish().expect("compression into memory")
}

#[derive(Clone, Debug)]
enum Kind {
    Group { linfo: usize, nlinks: usize },
    Dataset,
}

#[derive(Clone, Debug)]
struct Obj {
    oh: ObjHeader,
    kind: Kind,
}

/// An HDF5 file being written, as the HDF5 library holds it until it is closed.
pub struct Writer {
    space: Space,
    time: u32,
    objs: Vec<Obj>,
    gheap: GlobalHeap,
    btrees: Vec<ChunkBTree>,
    raw: Vec<(u64, Vec<u8>)>,
    deflate: Deflate,
}

impl Writer {
    /// `H5Fcreate` with library-version bounds 1.8: the superblock and the root group. Every
    /// object header gets `time` as its access, modification, change and birth time.
    pub fn new(time: u32, deflate: Deflate) -> Writer {
        let mut space = Space::new();
        let sb = space.alloc(Mem::Super, SUPERBLOCK);
        debug_assert_eq!(sb, 0);
        let mut w = Writer {
            space,
            time,
            objs: Vec::new(),
            gheap: GlobalHeap::default(),
            btrees: Vec::new(),
            raw: Vec::new(),
            deflate,
        };
        w.new_group();
        w
    }

    /// The root group.
    pub fn root(&self) -> GroupId {
        GroupId(0)
    }

    /// `H5G__obj_create_real` for a new-style group: a header sized for the link info, the
    /// group info and four links of 8-character names.
    fn new_group(&mut self) -> usize {
        let linfo = empty_link_info().raw_size() + 4;
        let ginfo = Native::GroupInfo.raw_size() + 4;
        let link = Native::Link {
            name: Vec::new(),
            addr: 0,
        }
        .raw_size()
            + 8
            + 4;
        let hint = linfo + ginfo + 4 * link;
        let mut oh = ObjHeader::create(&mut self.space, hint, self.time);
        let li = oh.append(&mut self.space, empty_link_info(), 0);
        oh.append(&mut self.space, Native::GroupInfo, CONSTANT);
        self.objs.push(Obj {
            oh,
            kind: Kind::Group {
                linfo: li,
                nlinks: 0,
            },
        });
        self.objs.len() - 1
    }

    /// `H5G_obj_insert` into a compact group: a link message, then the link info rewritten.
    fn link(&mut self, parent: GroupId, name: &str, target: usize) -> Result<()> {
        let addr = self.objs[target].oh.addr();
        let p = &mut self.objs[parent.0];
        let Kind::Group { linfo, nlinks } = p.kind else {
            return Err(Error::Write("not a group".into()));
        };
        if nlinks >= MAX_COMPACT {
            return Err(Error::Unsupported(
                "a group of more than 8 links (HDF5 stores it densely)".into(),
            ));
        }
        p.oh.append(
            &mut self.space,
            Native::Link {
                name: name.as_bytes().to_vec(),
                addr,
            },
            0,
        );
        p.oh.rewrite(linfo, empty_link_info());
        p.kind = Kind::Group {
            linfo,
            nlinks: nlinks + 1,
        };
        Ok(())
    }

    /// `H5Gcreate2`.
    pub fn create_group(&mut self, parent: GroupId, name: &str) -> Result<GroupId> {
        let g = self.new_group();
        self.link(parent, name, g)?;
        Ok(GroupId(g))
    }

    /// `H5Dcreate2` (`H5D__update_oh_info`): the dataset's header and messages, then its link.
    fn new_dataset(
        &mut self,
        parent: GroupId,
        name: &str,
        wtype: WType,
        n: u64,
        storage: Storage,
    ) -> Result<(usize, usize)> {
        let mut oh = ObjHeader::create(&mut self.space, DATASET_HEADER, self.time);
        oh.append(&mut self.space, Native::Dataspace { dims: vec![n] }, 0);
        oh.append(&mut self.space, Native::Datatype(wtype), CONSTANT);
        let (alloc_time, fill_time) = match (wtype, storage) {
            // Variable-length data: the fill value is written on allocation.
            (WType::VlenString, _) => (2, 0),
            (_, Storage::Contiguous) => (2, 2),
            (_, Storage::Chunked { .. }) => (3, 2),
        };
        oh.append(
            &mut self.space,
            Native::Fill {
                alloc_time,
                fill_time,
            },
            CONSTANT,
        );
        let layout = match storage {
            Storage::Contiguous => WLayout::Contiguous {
                addr: UNDEF,
                size: n * wtype.size() as u64,
            },
            Storage::Chunked { chunk, level } => {
                oh.append(&mut self.space, Native::Pline { level }, CONSTANT);
                WLayout::Chunked {
                    addr: UNDEF,
                    dims: vec![chunk as u32, wtype.size() as u32],
                }
            }
        };
        let li = oh.append(&mut self.space, Native::Layout(layout), 0);
        self.objs.push(Obj {
            oh,
            kind: Kind::Dataset,
        });
        let d = self.objs.len() - 1;
        self.link(parent, name, d)?;
        Ok((d, li))
    }

    /// A dataset of one variable-length string (`createDataSet` with `H5T_VARIABLE` `C_S1`
    /// and a dataspace of one element, then `write`).
    pub fn write_string(&mut self, parent: GroupId, name: &str, value: &[u8]) -> Result<()> {
        let (d, li) = self.new_dataset(parent, name, WType::VlenString, 1, Storage::Contiguous)?;
        let addr = self.space.alloc(Mem::Draw, 16);
        let id = self.gheap.insert(&mut self.space, value);
        let mut desc = Vec::with_capacity(16);
        desc.extend_from_slice(&(value.len() as u32).to_le_bytes());
        desc.extend_from_slice(&id.addr.to_le_bytes());
        desc.extend_from_slice(&id.index.to_le_bytes());
        self.raw.push((addr, desc));
        self.objs[d]
            .oh
            .rewrite(li, Native::Layout(WLayout::Contiguous { addr, size: 16 }));
        Ok(())
    }

    /// A one-dimensional floating-point dataset, written whole.
    pub fn write_floats(
        &mut self,
        parent: GroupId,
        name: &str,
        values: Floats<'_>,
        storage: Storage,
    ) -> Result<()> {
        let n = values.len();
        let wtype = values.wtype();
        if let Storage::Chunked { chunk, .. } = storage
            && (chunk == 0 || chunk > u64::from(u32::MAX))
        {
            return Err(Error::Write("all chunk dimensions must be positive".into()));
        }
        let (d, li) = self.new_dataset(parent, name, wtype, n as u64, storage)?;
        if n == 0 {
            return Ok(());
        }
        let esize = wtype.size();
        match storage {
            Storage::Contiguous => {
                let size = (n * esize) as u64;
                let addr = self.space.alloc(Mem::Draw, size);
                self.raw.push((addr, values.bytes(0, n)));
                self.objs[d]
                    .oh
                    .rewrite(li, Native::Layout(WLayout::Contiguous { addr, size }));
            }
            Storage::Chunked { chunk, level } => {
                let chunk = chunk as usize;
                let dims = vec![chunk as u32, esize as u32];
                let tree = ChunkBTree::create(&mut self.space, dims.clone());
                let tree_addr = tree.root;
                self.btrees.push(tree);
                let t = self.btrees.len() - 1;
                self.objs[d].oh.rewrite(
                    li,
                    Native::Layout(WLayout::Chunked {
                        addr: tree_addr,
                        dims,
                    }),
                );
                let chunk_bytes = chunk * esize;
                let nchunks = n.div_ceil(chunk);
                let cached = chunk_bytes <= CHUNK_CACHE;
                let mut pending = Vec::new();
                for k in 0..nchunks {
                    let start = k * chunk;
                    let end = (start + chunk).min(n);
                    let mut data = values.bytes(start, end);
                    data.resize(chunk_bytes, 0);
                    if cached {
                        pending.push((k, data));
                    } else {
                        self.flush_chunk(t, k, &data, level);
                    }
                }
                // H5Dclose flushes the chunk cache.
                for (k, data) in pending {
                    self.flush_chunk(t, k, &data, level);
                }
            }
        }
        Ok(())
    }

    /// `H5D__chunk_flush_entry`: filter, allocate, write and index one chunk.
    fn flush_chunk(&mut self, tree: usize, k: usize, data: &[u8], level: u32) {
        let z = (self.deflate)(data, level);
        let addr = self.space.alloc(Mem::Draw, z.len() as u64);
        let rec = ChunkRecord {
            scaled: vec![k as u64, 0],
            nbytes: z.len() as u32,
            mask: 0,
            addr,
        };
        self.raw.push((addr, z));
        self.btrees[tree].insert(&mut self.space, &rec);
    }

    /// `H5Fclose`: releases the free space, then writes everything at its address. Returns
    /// the file's bytes.
    pub fn finish(mut self) -> Vec<u8> {
        self.space.close();
        let eoa = self.space.eoa;
        let mut out = vec![0u8; eoa as usize];
        let mut put = |addr: u64, bytes: &[u8]| {
            let a = addr as usize;
            out[a..a + bytes.len()].copy_from_slice(bytes);
        };
        for obj in &self.objs {
            for (addr, image) in obj.oh.serialize() {
                put(addr, &image);
            }
        }
        for c in &self.gheap.collections {
            put(c.addr, &c.image);
        }
        for t in &self.btrees {
            for (addr, image) in t.serialize() {
                put(addr, &image);
            }
        }
        for (addr, bytes) in &self.raw {
            put(*addr, bytes);
        }
        // Superblock version 2.
        let mut sb = Vec::with_capacity(SUPERBLOCK as usize);
        sb.extend_from_slice(&crate::superblock::SIGNATURE);
        sb.extend_from_slice(&[2, 8, 8, 0]);
        sb.extend_from_slice(&0u64.to_le_bytes());
        sb.extend_from_slice(&UNDEF.to_le_bytes());
        sb.extend_from_slice(&eoa.to_le_bytes());
        sb.extend_from_slice(&self.objs[0].oh.addr().to_le_bytes());
        let sum = crate::checksum::metadata(&sb);
        sb.extend_from_slice(&sum.to_le_bytes());
        put(0, &sb);
        out
    }
}
