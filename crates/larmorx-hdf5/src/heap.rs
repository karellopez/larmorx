// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! Heaps, read only: local heaps (old-style group names, `src/H5HLcache.c`), global heap
//! collections (variable-length data, `src/H5HGcache.c`), and fractal heaps (dense groups'
//! links: `src/H5HFcache.c`, `src/H5HFdtable.c`, `src/H5HFdblock.c`, `src/H5HFman.c`,
//! `src/H5HFhuge.c`, `src/H5HFtiny.c`), as in HDF5 1.12.1.

use crate::btree;
use crate::error::{Result, corrupt, unsupported};
use crate::file::File;
use crate::source::{Cursor, UNDEF, log2_gen, verify_checksum};

/// A local heap's data segment.
pub(crate) fn local_heap(file: &File, addr: u64) -> Result<Vec<u8>> {
    let sb = file.superblock();
    let (a, s) = (sb.sizeof_addr, sb.sizeof_size);
    let head = file.read_at(addr, 8 + 2 * usize::from(s) + usize::from(a))?;
    let mut c = Cursor::new(&head, a, s, "a local heap");
    c.magic(b"HEAP")?;
    if c.u8()? != 0 {
        return Err(corrupt("wrong version number in local heap"));
    }
    c.skip(3)?;
    let size = c.length()?;
    let _free = c.length()?;
    let data = c.addr()?;
    if data == UNDEF {
        return Err(corrupt("local heap without a data segment"));
    }
    file.read_at(
        data,
        usize::try_from(size).map_err(|_| corrupt("local heap too large"))?,
    )
}

/// The NUL-terminated string at `offset` of a local heap's data segment.
pub(crate) fn heap_string(heap: &[u8], offset: u64) -> Result<Vec<u8>> {
    let off = usize::try_from(offset).map_err(|_| corrupt("bad local heap offset"))?;
    let rest = heap
        .get(off..)
        .ok_or_else(|| corrupt("local heap offset past the end of the heap"))?;
    let end = rest
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| corrupt("unterminated name in local heap"))?;
    Ok(rest[..end].to_vec())
}

/// One object of a global heap collection (`H5HG_read`).
pub(crate) fn global_object(file: &File, collection: u64, index: u32) -> Result<Vec<u8>> {
    let sb = file.superblock();
    let (a, s) = (sb.sizeof_addr, sb.sizeof_size);
    let head = file.read_at(collection, 8 + usize::from(s))?;
    let mut c = Cursor::new(&head, a, s, "a global heap collection");
    c.magic(b"GCOL")?;
    if c.u8()? != 1 {
        return Err(corrupt("wrong version number in global heap"));
    }
    c.skip(3)?;
    let size = c.length()?;
    if size < head.len() as u64 {
        return Err(corrupt("bad global heap collection size"));
    }
    let image = file.read_at(
        collection,
        usize::try_from(size).map_err(|_| corrupt("global heap collection too large"))?,
    )?;
    let mut p = head.len();
    let objhdr = 2 + 2 + 4 + usize::from(s);
    while p + objhdr <= image.len() {
        let mut c = Cursor::new(&image[p..], a, s, "a global heap object");
        let idx = u32::from(c.u16()?);
        let _nrefs = c.u16()?;
        c.skip(4)?;
        let osize = c.length()?;
        if idx == 0 {
            // The free space: the rest of the collection.
            break;
        }
        let osize = usize::try_from(osize).map_err(|_| corrupt("bad global heap object size"))?;
        let start = p + objhdr;
        if start + osize > image.len() {
            return Err(corrupt("global heap object past the end of its collection"));
        }
        if idx == index {
            return Ok(image[start..start + osize].to_vec());
        }
        p = start + osize.div_ceil(8) * 8;
    }
    Err(corrupt(format!(
        "global heap object {index} not found in the collection at {collection}"
    )))
}

/// A fractal heap's header, with the doubling table's derived values (`H5HF__dtable_init`).
pub(crate) struct FractalHeap {
    id_len: usize,
    filter_len: usize,
    checksum_dblocks: bool,
    max_man_size: u64,
    huge_bt2_addr: u64,
    width: u64,
    start_block_size: u64,
    root: u64,
    curr_root_rows: u64,
    first_row_bits: u32,
    max_direct_rows: u64,
    num_id_first_row: u64,
    row_block_size: Vec<u64>,
    heap_off_size: usize,
    heap_len_size: usize,
}

fn log2_of2(n: u64) -> Result<u32> {
    if n == 0 || !n.is_power_of_two() {
        return Err(corrupt("fractal heap block size is not a power of two"));
    }
    Ok(n.trailing_zeros())
}

impl FractalHeap {
    pub(crate) fn open(file: &File, addr: u64) -> Result<FractalHeap> {
        let sb = file.superblock();
        let (a, s) = (sb.sizeof_addr, sb.sizeof_size);
        let (au, su) = (usize::from(a), usize::from(s));
        let base =
            4 + 1 + 2 + 2 + 1 + 4 + su + au + su + au + 8 * su + 2 + su + su + 2 + 2 + au + 2;
        let prefix = file.read_at(addr, 9)?;
        let filter_len = usize::from(u16::from_le_bytes([prefix[7], prefix[8]]));
        let size =
            base + if filter_len > 0 {
                su + 4 + filter_len
            } else {
                0
            } + 4;
        let image = file.read_at(addr, size)?;
        verify_checksum(&image, "fractal heap header")?;
        let mut c = Cursor::new(&image, a, s, "a fractal heap header");
        c.magic(b"FRHP")?;
        if c.u8()? != 0 {
            return Err(corrupt("incorrect fractal heap header version"));
        }
        let id_len = usize::from(c.u16()?);
        let _filter_len = c.u16()?;
        let flags = c.u8()?;
        let max_man_size = u64::from(c.u32()?);
        let _huge_next_id = c.length()?;
        let huge_bt2_addr = c.addr()?;
        let _total_man_free = c.length()?;
        let _fs_addr = c.addr()?;
        for _ in 0..8 {
            c.length()?;
        }
        let width = u64::from(c.u16()?);
        let start_block_size = c.length()?;
        let max_direct_size = c.length()?;
        let max_index = u32::from(c.u16()?);
        let _start_root_rows = c.u16()?;
        let root = c.addr()?;
        let curr_root_rows = u64::from(c.u16()?);
        if width == 0 || !width.is_power_of_two() {
            return Err(corrupt("fractal heap table width is not a power of two"));
        }
        let start_bits = log2_of2(start_block_size)?;
        let first_row_bits = start_bits + log2_of2(width)?;
        if max_index < first_row_bits {
            return Err(corrupt("bad fractal heap maximum size"));
        }
        let max_root_rows = u64::from(max_index - first_row_bits) + 1;
        let max_direct_bits = log2_of2(max_direct_size)?;
        let max_direct_rows = u64::from(max_direct_bits.saturating_sub(start_bits)) + 2;
        let mut row_block_size = vec![start_block_size];
        let mut tmp = start_block_size;
        for _ in 1..max_root_rows.min(64) {
            row_block_size.push(tmp);
            tmp = tmp.saturating_mul(2);
        }
        let heap_off_size = (max_index as usize).div_ceil(8);
        let max_dir_blk_off_size = (max_direct_bits as usize).div_ceil(8);
        let heap_len_size = max_dir_blk_off_size.min(crate::source::limit_enc_size(max_man_size));
        Ok(FractalHeap {
            id_len,
            filter_len,
            checksum_dblocks: flags & 0x02 != 0,
            max_man_size,
            huge_bt2_addr,
            width,
            start_block_size,
            root,
            curr_root_rows,
            first_row_bits,
            max_direct_rows,
            num_id_first_row: start_block_size * width,
            row_block_size,
            heap_off_size,
            heap_len_size,
        })
    }

    pub(crate) fn id_len(&self) -> usize {
        self.id_len
    }

    fn lookup(&self, off: u64) -> (u64, u64) {
        if off < self.num_id_first_row {
            (0, off / self.start_block_size)
        } else {
            let high = log2_gen(off);
            let mask = 1u64 << high;
            let row = u64::from(high - self.first_row_bits) + 1;
            let size = self.row_block_size[row as usize];
            (row, (off - mask) / size)
        }
    }

    /// The object a heap ID names (`H5HF_read`).
    pub(crate) fn read(&self, file: &File, id: &[u8]) -> Result<Vec<u8>> {
        if id.is_empty() {
            return Err(corrupt("empty fractal heap ID"));
        }
        let flags = id[0];
        if flags >> 6 != 0 {
            return Err(corrupt("incorrect heap ID version"));
        }
        match (flags >> 4) & 0x03 {
            0 => self.read_managed(file, &id[1..]),
            1 => self.read_huge(file, &id[1..]),
            2 => {
                // Tiny: the object is in the ID itself (`H5HF__tiny_op_real`).
                let extended = self.id_len > 17;
                let (len, data) = if extended {
                    if id.len() < 2 {
                        return Err(corrupt("bad tiny heap ID"));
                    }
                    (
                        ((usize::from(flags & 0x0f)) << 8 | usize::from(id[1])) + 1,
                        &id[2..],
                    )
                } else {
                    (usize::from(flags & 0x0f) + 1, &id[1..])
                };
                data.get(..len)
                    .map(<[u8]>::to_vec)
                    .ok_or_else(|| corrupt("bad tiny heap ID"))
            }
            _ => Err(corrupt("unsupported heap ID type")),
        }
    }

    fn read_managed(&self, file: &File, rest: &[u8]) -> Result<Vec<u8>> {
        let sb = file.superblock();
        let mut c = Cursor::new(rest, sb.sizeof_addr, sb.sizeof_size, "a fractal heap ID");
        let off = c.uvar(self.heap_off_size)?;
        let len = c.uvar(self.heap_len_size)?;
        if len > self.max_man_size {
            return Err(corrupt("fractal heap object too large"));
        }
        if self.filter_len > 0 {
            return Err(unsupported("filtered fractal heaps"));
        }
        let (dblock, block_off, _block_size) = if self.curr_root_rows == 0 {
            (self.root, 0u64, self.start_block_size)
        } else {
            self.locate(file, off)?
        };
        if dblock == UNDEF {
            return Err(corrupt("fractal heap object in an unallocated block"));
        }
        let rel = off
            .checked_sub(block_off)
            .ok_or_else(|| corrupt("bad fractal heap offset"))?;
        let prefix = 4
            + 1
            + u64::from(sb.sizeof_addr)
            + self.heap_off_size as u64
            + if self.checksum_dblocks { 4 } else { 0 };
        if rel < prefix {
            return Err(corrupt("fractal heap object inside a block header"));
        }
        // Check the direct block's signature, then read the object.
        let head = file.read_at(dblock, 4)?;
        if head != b"FHDB" {
            return Err(corrupt("wrong fractal heap direct block signature"));
        }
        file.read_at(
            dblock + rel,
            usize::try_from(len).map_err(|_| corrupt("bad fractal heap object length"))?,
        )
    }

    /// The direct block holding heap offset `off`: its address, heap offset and size
    /// (`H5HF__man_dblock_locate`).
    fn locate(&self, file: &File, off: u64) -> Result<(u64, u64, u64)> {
        let sb = file.superblock();
        let (a, s) = (sb.sizeof_addr, sb.sizeof_size);
        let mut iblock_addr = self.root;
        let mut nrows = self.curr_root_rows;
        let mut iblock_off = 0u64;
        let (mut row, mut col) = self.lookup(off);
        for _ in 0..64 {
            let nents = (nrows * self.width) as usize;
            let size = 4 + 1 + usize::from(a) + self.heap_off_size + nents * usize::from(a) + 4;
            let image = file.read_at(iblock_addr, size)?;
            verify_checksum(&image, "fractal heap indirect block")?;
            let mut c = Cursor::new(&image, a, s, "a fractal heap indirect block");
            c.magic(b"FHIB")?;
            if c.u8()? != 0 {
                return Err(corrupt("incorrect fractal heap indirect block version"));
            }
            c.addr()?;
            let stored_off = c.uvar(self.heap_off_size)?;
            if stored_off != iblock_off {
                return Err(corrupt("fractal heap indirect block offset mismatch"));
            }
            let entry = (row * self.width + col) as usize;
            if entry >= nents {
                return Err(corrupt("fractal heap offset out of the indirect block"));
            }
            c.skip(entry * usize::from(a))?;
            let child = c.addr()?;
            if row < self.max_direct_rows {
                let block_size = self.row_block_size[row as usize];
                let block_off = iblock_off
                    + if row == 0 {
                        col * self.start_block_size
                    } else {
                        // row_block_off[row] = start * width * 2^(row - 1)
                        self.num_id_first_row * (1u64 << (row - 1)) + col * block_size
                    };
                return Ok((child, block_off, block_size));
            }
            let child_size = self.row_block_size[row as usize];
            let child_rows = u64::from(log2_gen(child_size) - self.first_row_bits) + 1;
            iblock_off += self.num_id_first_row * (1u64 << (row - 1)) + col * child_size;
            iblock_addr = child;
            nrows = child_rows;
            let rel = off - iblock_off;
            (row, col) = self.lookup(rel);
        }
        Err(corrupt("fractal heap too deep"))
    }

    fn read_huge(&self, file: &File, rest: &[u8]) -> Result<Vec<u8>> {
        // Huge objects are tracked by a v2 B-tree of type 1 (unfiltered, indirect IDs) or 3
        // (filtered); with directly stored IDs (types 2 and 4) the address is in the ID.
        let sb = file.superblock();
        let (a, s) = (sb.sizeof_addr, sb.sizeof_size);
        if self.filter_len > 0 {
            return Err(unsupported("filtered fractal heaps"));
        }
        let direct = self.id_len >= 1 + usize::from(a) + usize::from(s);
        let mut c = Cursor::new(rest, a, s, "a huge heap ID");
        if direct {
            let addr = c.addr()?;
            let len = c.length()?;
            return file.read_at(addr, len as usize);
        }
        let id = c.uvar(self.id_len - 1)?;
        let hdr = btree::v2_header(file, self.huge_bt2_addr, 1)?;
        for rec in btree::v2_records(file, &hdr)? {
            let mut r = Cursor::new(&rec, a, s, "a huge object record");
            let addr = r.addr()?;
            let len = r.length()?;
            let rid = r.length()?;
            if rid == id {
                return file.read_at(addr, len as usize);
            }
        }
        Err(corrupt("huge fractal heap object not found"))
    }
}
