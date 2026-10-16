// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! Reading a dataset's stored elements: compact, contiguous and chunked layouts, chunks
//! indexed by a version-1 B-tree (`src/H5Dbtree.c`), a single chunk (`src/H5Dsingle.c`),
//! implicitly (`src/H5Dnone.c`), by a fixed array (`src/H5Dfarray.c`, `src/H5FAcache.c`) or a
//! version-2 B-tree (`src/H5Dbt2.c`); unallocated storage reads as the fill value
//! (`src/H5Dfill.c`). As in HDF5 1.12.1.

use rayon::prelude::*;

use crate::btree;
use crate::datatype::Datatype;
use crate::error::{Error, Result, corrupt, unsupported};
use crate::file::File;
use crate::filters;
use crate::message::{self, ChunkIndex, Dataspace, FillValue, Filter, Layout};
use crate::object::{ObjectHeader, flag, msg};
use crate::source::{Cursor, UNDEF, log2_gen, verify_checksum};

/// What a dataset's object header says about it.
#[derive(Clone, Debug)]
pub struct DatasetInfo {
    pub space: Dataspace,
    pub dtype: Datatype,
    pub layout: Layout,
    pub pipeline: Vec<Filter>,
    pub fill: FillValue,
}

/// `H5O__dset_isa`: a datatype and a dataspace message.
pub(crate) fn is_dataset(oh: &ObjectHeader) -> bool {
    oh.has(msg::DATATYPE) && oh.has(msg::DATASPACE)
}

/// Decodes a message that may be shared (stored in another object header, as committed
/// datatypes are).
fn message_data(file: &File, oh: &ObjectHeader, type_id: u16) -> Result<Option<Vec<u8>>> {
    let Some(m) = oh.find(type_id) else {
        return Ok(None);
    };
    if m.flags & flag::SHARED == 0 {
        return Ok(Some(m.data.clone()));
    }
    let sb = file.superblock();
    let at = message::shared_location(&mut Cursor::new(
        &m.data,
        sb.sizeof_addr,
        sb.sizeof_size,
        "a shared message",
    ))?;
    let other = file.object_header(at)?;
    message_data(file, &other, type_id)
}

pub(crate) fn info(file: &File, oh: &ObjectHeader) -> Result<DatasetInfo> {
    let sb = file.superblock();
    let (a, s) = (sb.sizeof_addr, sb.sizeof_size);
    let need = |t: u16, what: &str| -> Result<Vec<u8>> {
        message_data(file, oh, t)?
            .ok_or_else(|| corrupt(format!("dataset without a {what} message")))
    };
    let dtype = message::datatype(&mut Cursor::new(
        &need(msg::DATATYPE, "datatype")?,
        a,
        s,
        "a datatype message",
    ))?;
    let space = message::dataspace(&mut Cursor::new(
        &need(msg::DATASPACE, "dataspace")?,
        a,
        s,
        "a dataspace message",
    ))?;
    let layout = message::layout(&mut Cursor::new(
        &need(msg::LAYOUT, "layout")?,
        a,
        s,
        "a layout message",
    ))?;
    let pipeline = match message_data(file, oh, msg::PIPELINE)? {
        Some(d) => message::pipeline(&mut Cursor::new(&d, a, s, "a filter pipeline message"))?,
        None => Vec::new(),
    };
    let fill = if let Some(d) = message_data(file, oh, msg::FILL)? {
        message::fill_new(&mut Cursor::new(&d, a, s, "a fill value message"))?
    } else if let Some(d) = message_data(file, oh, msg::FILL_OLD)? {
        message::fill_old(&mut Cursor::new(&d, a, s, "a fill value message"))?
    } else {
        FillValue::default()
    };
    if oh.has(msg::EXTERNAL_FILES) {
        return Err(unsupported("datasets stored in external files"));
    }
    Ok(DatasetInfo {
        space,
        dtype,
        layout,
        pipeline,
        fill,
    })
}

/// One stored element of the fill value: the defined value, or zeros.
fn fill_element(info: &DatasetInfo, elem: usize) -> Vec<u8> {
    match &info.fill.value {
        Some(v) if v.len() == elem => v.clone(),
        _ => vec![0u8; elem],
    }
}

/// A chunk as the dataset's index lists it (`H5Dget_chunk_info`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chunk {
    /// Scaled coordinates (offsets divided by the chunk dimensions).
    pub scaled: Vec<u64>,
    /// Where the chunk is stored.
    pub addr: u64,
    /// Stored size in bytes (after the filters).
    pub size: u64,
    /// The filters skipped for this chunk (bit i: filter i).
    pub mask: u32,
}

/// What reading a chunked dataset needs: its extent, the chunk dimensions and grid, the size
/// of an unfiltered chunk.
struct ChunkGeometry {
    dims: Vec<u64>,
    cdims: Vec<u64>,
    grid: Vec<u64>,
    chunk_bytes: usize,
}

fn chunk_geometry(info: &DatasetInfo, cdims_all: &[u64]) -> Result<ChunkGeometry> {
    let elem = info.dtype.size();
    let rank = info.space.rank();
    let dims: Vec<u64> = match &info.space {
        Dataspace::Simple { dims, .. } => dims.clone(),
        _ => vec![1; rank],
    };
    if cdims_all.len() != rank + 1 {
        return Err(corrupt(
            "chunked layout's dimensionality does not match the dataspace",
        ));
    }
    let cdims = &cdims_all[..rank];
    if cdims_all[rank] as usize != elem {
        // HDF5 checks the chunk's element dimension against the datatype size when the
        // dataset is opened.
        return Err(corrupt("chunk element size does not match the datatype"));
    }
    if cdims.contains(&0) {
        return Err(corrupt("chunk dimension must be positive"));
    }
    let chunk_elems: u64 = cdims.iter().product();
    let chunk_bytes =
        usize::try_from(chunk_elems * elem as u64).map_err(|_| corrupt("chunk too large"))?;
    let grid: Vec<u64> = dims
        .iter()
        .zip(cdims)
        .map(|(&d, &c)| d.div_ceil(c))
        .collect();
    Ok(ChunkGeometry {
        dims,
        cdims: cdims.to_vec(),
        grid,
        chunk_bytes,
    })
}

/// The stored chunks of a chunked dataset, as its index lists them (none for other layouts).
pub(crate) fn chunks(file: &File, info: &DatasetInfo) -> Result<Vec<Chunk>> {
    let Layout::Chunked {
        dims, index, addr, ..
    } = &info.layout
    else {
        return Ok(Vec::new());
    };
    let g = chunk_geometry(info, dims)?;
    if g.dims.iter().product::<u64>() == 0 {
        return Ok(Vec::new());
    }
    list_chunks(
        file,
        info,
        index,
        *addr,
        &g.cdims,
        &g.grid,
        g.chunk_bytes,
        !info.pipeline.is_empty(),
    )
}

/// Every stored element of the dataset, row-major, `dtype.size()` bytes each.
pub(crate) fn read_raw(file: &File, info: &DatasetInfo, n_threads: usize) -> Result<Vec<u8>> {
    let elem = info.dtype.size();
    if elem == 0 {
        return Err(corrupt("datatype of size 0"));
    }
    let n = usize::try_from(info.space.npoints()).map_err(|_| corrupt("dataset too large"))?;
    let total = n
        .checked_mul(elem)
        .ok_or_else(|| corrupt("size of dataset's storage overflowed"))?;
    match &info.layout {
        Layout::Compact { data, .. } => {
            if data.len() < total {
                return Err(corrupt(
                    "compact dataset's data is smaller than its dataspace",
                ));
            }
            Ok(data[..total].to_vec())
        }
        Layout::Contiguous { addr, .. } => {
            if *addr == UNDEF {
                return Ok(fill_element(info, elem).repeat(n));
            }
            file.read_at(*addr, total)
        }
        Layout::Chunked {
            dims, index, addr, ..
        } => read_chunked(file, info, dims, index, *addr, n_threads),
        Layout::Virtual => Err(unsupported("virtual datasets")),
    }
}

fn read_chunked(
    file: &File,
    info: &DatasetInfo,
    cdims_all: &[u64],
    index: &ChunkIndex,
    idx_addr: u64,
    n_threads: usize,
) -> Result<Vec<u8>> {
    let elem = info.dtype.size();
    let ChunkGeometry {
        dims,
        cdims,
        grid,
        chunk_bytes,
    } = chunk_geometry(info, cdims_all)?;
    let cdims = &cdims[..];
    let n = dims.iter().product::<u64>() as usize;
    let mut out = fill_element(info, elem).repeat(n);
    if n == 0 {
        return Ok(out);
    }
    let filtered = !info.pipeline.is_empty();
    let chunks = list_chunks(
        file,
        info,
        index,
        idx_addr,
        cdims,
        &grid,
        chunk_bytes,
        filtered,
    )?;
    // Decompress in parallel; each chunk lands in its own place, so the result does not
    // depend on the thread count.
    let decoded: Vec<(Vec<u64>, Vec<u8>)> = larmorx_core::parallel::with_threads(n_threads, || {
        chunks
            .par_iter()
            .map(|c| -> Result<(Vec<u64>, Vec<u8>)> {
                let size = usize::try_from(c.size).map_err(|_| corrupt("chunk too large"))?;
                let raw = file.read_at(c.addr, size)?;
                let data = if filtered {
                    filters::unfilter(&info.pipeline, c.mask, raw)?
                } else {
                    raw
                };
                if data.len() < chunk_bytes {
                    // HDF5 would use the rest of its buffer, uninitialised.
                    return Err(Error::Filter(format!(
                        "a chunk holds {} bytes after its filters, fewer than the {chunk_bytes} of a chunk",
                        data.len()
                    )));
                }
                Ok((c.scaled.clone(), data))
            })
            .collect::<Result<Vec<_>>>()
    })
    .map_err(|e| Error::Threads(e.to_string()))??;
    // Copy each chunk's part inside the dataset.
    for (scaled, data) in decoded {
        copy_chunk(&mut out, &dims, cdims, &scaled, &data, elem);
    }
    Ok(out)
}

fn copy_chunk(
    out: &mut [u8],
    dims: &[u64],
    cdims: &[u64],
    scaled: &[u64],
    data: &[u8],
    elem: usize,
) {
    let rank = dims.len();
    if rank == 0 {
        out[..elem].copy_from_slice(&data[..elem]);
        return;
    }
    let start: Vec<u64> = scaled.iter().zip(cdims).map(|(&s, &c)| s * c).collect();
    if start.iter().zip(dims).any(|(&s, &d)| s >= d) {
        return;
    }
    let extent: Vec<u64> = (0..rank)
        .map(|k| cdims[k].min(dims[k] - start[k]))
        .collect();
    // Iterate over the chunk's rows (all but the last dimension).
    let row_len = extent[rank - 1] as usize * elem;
    let mut idx = vec![0u64; rank - 1];
    loop {
        // Offsets of the row in the chunk and in the dataset.
        let mut src = 0u64;
        let mut dst = 0u64;
        for k in 0..rank - 1 {
            src = src * cdims[k] + idx[k];
            dst = dst * dims[k] + start[k] + idx[k];
        }
        let src = (src * cdims[rank - 1]) as usize * elem;
        let dst = (dst * dims[rank - 1] + start[rank - 1]) as usize * elem;
        out[dst..dst + row_len].copy_from_slice(&data[src..src + row_len]);
        // Next row.
        let mut k = rank - 1;
        loop {
            if k == 0 {
                return;
            }
            k -= 1;
            idx[k] += 1;
            if idx[k] < extent[k] {
                break;
            }
            idx[k] = 0;
        }
    }
}

/// Scaled coordinates of linear chunk index `i` in a row-major grid.
fn unravel(mut i: u64, grid: &[u64]) -> Vec<u64> {
    let mut out = vec![0u64; grid.len()];
    for k in (0..grid.len()).rev() {
        out[k] = i % grid[k];
        i /= grid[k];
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn list_chunks(
    file: &File,
    info: &DatasetInfo,
    index: &ChunkIndex,
    idx_addr: u64,
    cdims: &[u64],
    grid: &[u64],
    chunk_bytes: usize,
    filtered: bool,
) -> Result<Vec<Chunk>> {
    let sb = file.superblock();
    let (a, s) = (sb.sizeof_addr, sb.sizeof_size);
    let rank = cdims.len();
    let nchunks: u64 = grid.iter().product();
    if idx_addr == UNDEF {
        return Ok(Vec::new());
    }
    match index {
        ChunkIndex::BTreeV1 => {
            let key_size = 4 + 4 + (rank + 1) * 8;
            let mut out = Vec::new();
            for e in btree::v1_leaves(file, idx_addr, 1, key_size)? {
                let mut c = Cursor::new(&e.key, a, s, "a chunk B-tree key");
                let size = u64::from(c.u32()?);
                let mask = c.u32()?;
                let mut scaled = Vec::with_capacity(rank);
                for &cd in cdims {
                    let off = c.u64()?;
                    if off % cd != 0 {
                        return Err(corrupt("chunk offset is not a multiple of the chunk size"));
                    }
                    scaled.push(off / cd);
                }
                out.push(Chunk {
                    scaled,
                    addr: e.child,
                    size,
                    mask,
                });
            }
            Ok(out)
        }
        ChunkIndex::Single { filtered: f } => {
            let (size, mask) = match f {
                Some((nbytes, mask)) => (*nbytes, *mask),
                None => (chunk_bytes as u64, 0),
            };
            Ok(vec![Chunk {
                scaled: vec![0; rank],
                addr: idx_addr,
                size,
                mask,
            }])
        }
        ChunkIndex::Implicit => {
            if filtered {
                return Err(corrupt("implicit chunk index with filters"));
            }
            Ok((0..nchunks)
                .map(|i| Chunk {
                    scaled: unravel(i, grid),
                    addr: idx_addr + i * chunk_bytes as u64,
                    size: chunk_bytes as u64,
                    mask: 0,
                })
                .collect())
        }
        ChunkIndex::FixedArray { .. } => fixed_array(file, idx_addr, grid, chunk_bytes, filtered),
        ChunkIndex::BTreeV2 { .. } => {
            let btype = if filtered { 11 } else { 10 };
            let hdr = btree::v2_header(file, idx_addr, btype)?;
            let mut out = Vec::new();
            let csl = chunk_size_len(chunk_bytes);
            for rec in btree::v2_records(file, &hdr)? {
                let mut c = Cursor::new(&rec, a, s, "a chunk index record");
                let addr = c.addr()?;
                let (size, mask) = if filtered {
                    (c.uvar(csl)?, c.u32()?)
                } else {
                    (chunk_bytes as u64, 0)
                };
                let mut scaled = Vec::with_capacity(rank);
                for _ in 0..rank {
                    scaled.push(c.u64()?);
                }
                out.push(Chunk {
                    scaled,
                    addr,
                    size,
                    mask,
                });
            }
            Ok(out)
        }
        ChunkIndex::ExtensibleArray { .. } => {
            extensible_array(file, info, idx_addr, cdims, chunk_bytes, filtered)
        }
    }
}

/// Bytes encoding a filtered chunk's size: one more than its unfiltered size needs, at most 8
/// (`H5D__farray_idx_create`, `H5D__bt2_idx_create`).
/// The chunks of an extensible-array index (`src/H5Dearray.c`, `src/H5EA*.c`): element `i` of
/// the array is the chunk whose coordinates, with the unlimited dimension moved first
/// ("swizzled", `H5VM_swizzle_coords`), have the linear index `i`. The array keeps its first
/// elements in the index block, then data blocks of growing size, the first ones addressed from
/// the index block, the others through super blocks; large data blocks are split into pages.
/// Elements up to the largest index set are listed (`H5D__earray_idx_iterate`).
fn extensible_array(
    file: &File,
    info: &DatasetInfo,
    addr: u64,
    cdims: &[u64],
    chunk_bytes: usize,
    filtered: bool,
) -> Result<Vec<Chunk>> {
    let sb = file.superblock();
    let (a, s) = (sb.sizeof_addr, sb.sizeof_size);
    let ea = ExtensibleArray::open(file, addr, filtered, a, s)?;
    // The grid of chunks, the unlimited dimension first.
    let rank = cdims.len();
    let (dims, max_dims) = match &info.space {
        Dataspace::Simple { dims, max_dims } => (
            dims.clone(),
            max_dims.clone().unwrap_or_else(|| dims.clone()),
        ),
        _ => {
            return Err(corrupt(
                "extensible array index of a dataset without dimensions",
            ));
        }
    };
    let unlim = max_dims
        .iter()
        .position(|&m| m == UNDEF)
        .ok_or_else(|| corrupt("extensible array index without an unlimited dimension"))?;
    let mut order: Vec<usize> = (0..rank).collect();
    order.remove(unlim);
    order.insert(0, unlim);
    let mut down = vec![1u64; rank];
    for k in (0..rank.saturating_sub(1)).rev() {
        let d = order[k + 1];
        down[k] = down[k + 1].saturating_mul(max_dims[d].div_ceil(cdims[d]));
    }
    let grid: Vec<u64> = dims
        .iter()
        .zip(cdims)
        .map(|(&d, &c)| d.div_ceil(c))
        .collect();
    let csl = chunk_size_len(chunk_bytes);
    let mut out = Vec::new();
    let mut cache = EaCache::default();
    for idx in 0..ea.max_idx_set {
        let Some(raw) = ea.element(file, idx, &mut cache)? else {
            continue;
        };
        let mut c = Cursor::new(&raw, a, s, "an extensible array element");
        let caddr = c.addr()?;
        if caddr == UNDEF {
            continue;
        }
        let (size, mask) = if filtered {
            (c.uvar(csl)?, c.u32()?)
        } else {
            (chunk_bytes as u64, 0)
        };
        let mut swizzled = vec![0u64; rank];
        let mut rest = idx;
        for k in 0..rank {
            swizzled[k] = rest / down[k];
            rest %= down[k];
        }
        let mut scaled = vec![0u64; rank];
        for (k, &d) in order.iter().enumerate() {
            scaled[d] = swizzled[k];
        }
        if scaled.iter().zip(&grid).all(|(s, g)| s < g) {
            out.push(Chunk {
                scaled,
                addr: caddr,
                size,
                mask,
            });
        }
    }
    Ok(out)
}

/// An extensible array's header and index block (`H5EA__cache_hdr_deserialize`,
/// `H5EA__hdr_init`, `H5EA__cache_iblock_deserialize`).
struct ExtensibleArray {
    addr: u64,
    sizeof_addr: u8,
    sizeof_size: u8,
    elsize: usize,
    idx_blk_elmts: u64,
    data_blk_min_elmts: u64,
    page_nelmts: u64,
    arr_off_size: usize,
    max_idx_set: u64,
    /// Per super block: (number of data blocks, elements per data block, first element,
    /// first data block).
    sblk: Vec<(u64, u64, u64, u64)>,
    /// Super blocks whose data blocks the index block addresses.
    ib_nsblks: usize,
    iblock_elmts: Vec<u8>,
    dblk_addrs: Vec<u64>,
    sblk_addrs: Vec<u64>,
}

/// Blocks read once each: data blocks and pages by address (`None`: a page never
/// written), super blocks by index (their data block addresses and page-init bits).
#[derive(Default)]
struct EaCache {
    blocks: std::collections::HashMap<u64, Option<Vec<u8>>>,
    sblocks: std::collections::HashMap<usize, (Vec<u64>, Vec<u8>)>,
}

impl ExtensibleArray {
    fn open(file: &File, addr: u64, filtered: bool, a: u8, s: u8) -> Result<ExtensibleArray> {
        let (au, su) = (usize::from(a), usize::from(s));
        let hsize = 4 + 1 + 1 + 6 + 6 * su + au + 4;
        let image = file.read_at(addr, hsize)?;
        verify_checksum(&image, "extensible array header")?;
        let mut c = Cursor::new(&image, a, s, "an extensible array header");
        c.magic(b"EAHD")?;
        if c.u8()? != 0 {
            return Err(corrupt("wrong extensible array header version"));
        }
        if c.u8()? != u8::from(filtered) {
            return Err(corrupt("extensible array class does not match the filters"));
        }
        let elsize = usize::from(c.u8()?);
        let max_nelmts_bits = u32::from(c.u8()?);
        let idx_blk_elmts = u64::from(c.u8()?);
        let data_blk_min_elmts = u64::from(c.u8()?);
        let sup_blk_min_data_ptrs = u64::from(c.u8()?);
        let page_bits = u32::from(c.u8()?);
        for _ in 0..4 {
            c.length()?;
        }
        let max_idx_set = c.length()?;
        c.length()?;
        let iblock_addr = c.addr()?;
        if !data_blk_min_elmts.is_power_of_two()
            || !sup_blk_min_data_ptrs.is_power_of_two()
            || max_nelmts_bits >= 64
            || page_bits >= 32
            || data_blk_min_elmts.ilog2() > max_nelmts_bits
            || elsize < au
        {
            return Err(corrupt("invalid extensible array creation parameter"));
        }
        let nsblks = 1 + (max_nelmts_bits - data_blk_min_elmts.ilog2()) as usize;
        let mut sblk = Vec::with_capacity(nsblks);
        let (mut start_idx, mut start_dblk) = (0u64, 0u64);
        for u in 0..nsblks {
            let ndblks = 1u64 << (u / 2);
            let dblk_nelmts = (1u64 << u.div_ceil(2)) * data_blk_min_elmts;
            sblk.push((ndblks, dblk_nelmts, start_idx, start_dblk));
            start_idx = start_idx.saturating_add(ndblks.saturating_mul(dblk_nelmts));
            start_dblk += ndblks;
        }
        let ib_nsblks = 2 * sup_blk_min_data_ptrs.ilog2() as usize;
        let ndblk_addrs = 2 * (sup_blk_min_data_ptrs as usize - 1);
        let nsblk_addrs = nsblks.saturating_sub(ib_nsblks);
        let mut ea = ExtensibleArray {
            addr,
            sizeof_addr: a,
            sizeof_size: s,
            elsize,
            idx_blk_elmts,
            data_blk_min_elmts,
            page_nelmts: 1u64 << page_bits,
            arr_off_size: max_nelmts_bits.div_ceil(8) as usize,
            max_idx_set,
            sblk,
            ib_nsblks,
            iblock_elmts: Vec::new(),
            dblk_addrs: Vec::new(),
            sblk_addrs: Vec::new(),
        };
        if iblock_addr == UNDEF {
            ea.max_idx_set = 0;
            return Ok(ea);
        }
        let size = 6 + au + idx_blk_elmts as usize * elsize + (ndblk_addrs + nsblk_addrs) * au + 4;
        let image = file.read_at(iblock_addr, size)?;
        verify_checksum(&image, "extensible array index block")?;
        let mut c = Cursor::new(&image, a, s, "an extensible array index block");
        c.magic(b"EAIB")?;
        c.skip(2)?;
        if c.addr()? != addr {
            return Err(corrupt("extensible array index block of another array"));
        }
        ea.iblock_elmts = c.bytes(idx_blk_elmts as usize * elsize)?.to_vec();
        for _ in 0..ndblk_addrs {
            ea.dblk_addrs.push(c.addr()?);
        }
        for _ in 0..nsblk_addrs {
            ea.sblk_addrs.push(c.addr()?);
        }
        Ok(ea)
    }

    fn dblock_prefix(&self) -> usize {
        6 + usize::from(self.sizeof_addr) + self.arr_off_size + 4
    }

    /// A data block's elements (`H5EA__cache_dblock_deserialize`, unpaged).
    fn read_dblock(&self, file: &File, at: u64, nelmts: u64) -> Result<Vec<u8>> {
        let prefix = self.dblock_prefix() - 4;
        let len = prefix + nelmts as usize * self.elsize;
        let image = file.read_at(at, len + 4)?;
        verify_checksum(&image, "extensible array data block")?;
        let mut c = Cursor::new(
            &image,
            self.sizeof_addr,
            self.sizeof_size,
            "an extensible array data block",
        );
        c.magic(b"EADB")?;
        c.skip(2)?;
        if c.addr()? != self.addr {
            return Err(corrupt("extensible array data block of another array"));
        }
        Ok(image[prefix..len].to_vec())
    }

    /// Element `idx` as stored, or `None` where nothing was ever stored (`H5EA__lookup_elmt`
    /// with read-only access).
    fn element(&self, file: &File, idx: u64, cache: &mut EaCache) -> Result<Option<Vec<u8>>> {
        let e = self.elsize;
        if idx < self.idx_blk_elmts {
            let i = idx as usize;
            return Ok(Some(self.iblock_elmts[i * e..(i + 1) * e].to_vec()));
        }
        let rel = idx - self.idx_blk_elmts;
        let si = log2_gen(rel / self.data_blk_min_elmts + 1) as usize;
        let (ndblks, dblk_nelmts, sstart, sdblk) = *self
            .sblk
            .get(si)
            .ok_or_else(|| corrupt("extensible array index beyond its super blocks"))?;
        let elmt = rel - sstart;
        let k = elmt / dblk_nelmts;
        let within = elmt % dblk_nelmts;
        let (block_at, offset) = if si < self.ib_nsblks {
            let at = *self
                .dblk_addrs
                .get((sdblk + k) as usize)
                .ok_or_else(|| corrupt("extensible array data block beyond the index block"))?;
            if at == UNDEF {
                return Ok(None);
            }
            if dblk_nelmts > self.page_nelmts {
                return Err(unsupported(
                    "paged extensible array data blocks addressed by the index block",
                ));
            }
            if let std::collections::hash_map::Entry::Vacant(v) = cache.blocks.entry(at) {
                v.insert(Some(self.read_dblock(file, at, dblk_nelmts)?));
            }
            (at, within)
        } else {
            let saddr = self.sblk_addrs[si - self.ib_nsblks];
            if saddr == UNDEF {
                return Ok(None);
            }
            let npages = if dblk_nelmts > self.page_nelmts {
                dblk_nelmts / self.page_nelmts
            } else {
                0
            };
            if let std::collections::hash_map::Entry::Vacant(v) = cache.sblocks.entry(si) {
                // H5EA__cache_sblock_deserialize.
                let init_size = (npages as usize).div_ceil(8);
                let au = usize::from(self.sizeof_addr);
                let size = 6 + au + self.arr_off_size + ndblks as usize * (init_size + au) + 4;
                let image = file.read_at(saddr, size)?;
                verify_checksum(&image, "extensible array super block")?;
                let mut c = Cursor::new(
                    &image,
                    self.sizeof_addr,
                    self.sizeof_size,
                    "an extensible array super block",
                );
                c.magic(b"EASB")?;
                c.skip(2)?;
                if c.addr()? != self.addr {
                    return Err(corrupt("extensible array super block of another array"));
                }
                c.skip(self.arr_off_size)?;
                let init = c.bytes(ndblks as usize * init_size)?.to_vec();
                let mut addrs = Vec::with_capacity(ndblks as usize);
                for _ in 0..ndblks {
                    addrs.push(c.addr()?);
                }
                v.insert((addrs, init));
            }
            let (addrs, init) = &cache.sblocks[&si];
            let at = addrs[k as usize];
            if at == UNDEF {
                return Ok(None);
            }
            if npages == 0 {
                if let std::collections::hash_map::Entry::Vacant(v) = cache.blocks.entry(at) {
                    v.insert(Some(self.read_dblock(file, at, dblk_nelmts)?));
                }
                (at, within)
            } else {
                let page = within / self.page_nelmts;
                // H5VM_bit_get over the super block's whole bitmask.
                let bit = k as usize * npages as usize + page as usize;
                let page_size = self.page_nelmts * e as u64 + 4;
                let page_at = at + self.dblock_prefix() as u64 + page * page_size;
                let initialised = init[bit / 8] & (0x80 >> (bit % 8)) != 0;
                if let std::collections::hash_map::Entry::Vacant(v) = cache.blocks.entry(page_at) {
                    let b = if initialised {
                        let image = file.read_at(page_at, page_size as usize)?;
                        verify_checksum(&image, "extensible array data block page")?;
                        Some(image[..image.len() - 4].to_vec())
                    } else {
                        None
                    };
                    v.insert(b);
                }
                (page_at, within % self.page_nelmts)
            }
        };
        Ok(cache.blocks[&block_at].as_ref().map(|b| {
            let o = offset as usize;
            b[o * e..(o + 1) * e].to_vec()
        }))
    }
}

fn chunk_size_len(chunk_bytes: usize) -> usize {
    (1 + ((crate::source::log2_gen(chunk_bytes as u64) + 8) / 8) as usize).min(8)
}

/// The chunks of a fixed array index (`H5FA` header, data block and pages).
fn fixed_array(
    file: &File,
    addr: u64,
    grid: &[u64],
    chunk_bytes: usize,
    filtered: bool,
) -> Result<Vec<Chunk>> {
    let sb = file.superblock();
    let (a, s) = (sb.sizeof_addr, sb.sizeof_size);
    let (au, su) = (usize::from(a), usize::from(s));
    let hsize = 4 + 1 + 1 + 1 + 1 + su + au + 4;
    let image = file.read_at(addr, hsize)?;
    verify_checksum(&image, "fixed array header")?;
    let mut c = Cursor::new(&image, a, s, "a fixed array header");
    c.magic(b"FAHD")?;
    if c.u8()? != 0 {
        return Err(corrupt("wrong fixed array header version"));
    }
    let class = c.u8()?;
    if class != u8::from(filtered) {
        return Err(corrupt("fixed array class does not match the filters"));
    }
    let elsize = usize::from(c.u8()?);
    let page_bits = u32::from(c.u8()?);
    let nelmts = c.length()?;
    let dblk = c.addr()?;
    let mut out = Vec::new();
    if dblk == UNDEF || nelmts == 0 {
        return Ok(out);
    }
    if page_bits >= 64 {
        return Err(corrupt("bad fixed array page size"));
    }
    let page_nelmts = 1u64 << page_bits;
    let paged = nelmts > page_nelmts;
    let npages = if paged {
        nelmts.div_ceil(page_nelmts)
    } else {
        0
    };
    let bitmap_len = if paged {
        npages.div_ceil(8) as usize
    } else {
        0
    };
    let prefix = 4 + 1 + 1 + au + bitmap_len;
    let decode = |c: &mut Cursor, i: u64, out: &mut Vec<Chunk>| -> Result<()> {
        let caddr = c.addr()?;
        let (size, mask) = if filtered {
            let csl = elsize
                .checked_sub(au + 4)
                .ok_or_else(|| corrupt("bad fixed array element size"))?;
            (c.uvar(csl)?, c.u32()?)
        } else {
            (chunk_bytes as u64, 0)
        };
        if caddr != UNDEF && i < grid.iter().product::<u64>() {
            out.push(Chunk {
                scaled: unravel(i, grid),
                addr: caddr,
                size,
                mask,
            });
        }
        Ok(())
    };
    if !paged {
        let size = prefix + nelmts as usize * elsize + 4;
        let image = file.read_at(dblk, size)?;
        verify_checksum(&image, "fixed array data block")?;
        let mut c = Cursor::new(&image, a, s, "a fixed array data block");
        c.magic(b"FADB")?;
        c.skip(2 + au)?;
        for i in 0..nelmts {
            decode(&mut c, i, &mut out)?;
        }
        return Ok(out);
    }
    let image = file.read_at(dblk, prefix + 4)?;
    verify_checksum(&image, "fixed array data block")?;
    let mut c = Cursor::new(&image, a, s, "a fixed array data block");
    c.magic(b"FADB")?;
    c.skip(2 + au)?;
    let bitmap = c.bytes(bitmap_len)?.to_vec();
    let mut page_addr = dblk + (prefix + 4) as u64;
    for p in 0..npages {
        let count = if p + 1 == npages && nelmts % page_nelmts != 0 {
            nelmts % page_nelmts
        } else {
            page_nelmts
        };
        let psize = count as usize * elsize + 4;
        if bitmap[(p / 8) as usize] & (0x80 >> (p % 8)) != 0 {
            let image = file.read_at(page_addr, psize)?;
            verify_checksum(&image, "fixed array data block page")?;
            let mut c = Cursor::new(&image, a, s, "a fixed array data block page");
            for k in 0..count {
                decode(&mut c, p * page_nelmts + k, &mut out)?;
            }
        }
        page_addr += psize as u64;
    }
    Ok(out)
}
