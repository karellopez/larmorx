// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! Dense link storage as HDF5 1.12.1 creates it when a new-style group gets its ninth link
//! (`H5G__dense_create`, `H5G__dense_insert`, `src/H5Gdense.c`): each link, encoded as a link
//! message, is an object of a fractal heap (`H5HF_create`, `H5HF__man_insert`,
//! `H5HF__man_dblock_new`, `H5HF__man_dblock_create`, `H5HF__sect_single_reduce`,
//! `src/H5HF*.c`) whose free space a persistent free-space manager tracks (`H5FS_create`,
//! `H5FS_close`, `H5FS_delete`, `src/H5FS*.c`), and a version-2 B-tree indexes the links by the
//! lookup3 hash of their names (`H5B2_create`, `H5B2__insert`, `H5B2__create_leaf`,
//! `src/H5B2*.c`, `H5G__dense_btree2_name_*`, `src/H5Gbtree2.c`).
//!
//! The heap's root direct block and its free-space section info get temporary addresses when
//! they are created; HDF5 gives them real file space when the metadata cache is flushed at
//! close, the section info first (`H5FS__cache_hdr_pre_serialize`), then the block
//! (`H5HF__cache_dblock_pre_serialize`) — [`Dense::place_sections`] and
//! [`Dense::place_block`].
//!
//! Covered: the heap's first direct block (512 bytes) and the B-tree's root leaf: about 37
//! links with names of one or two characters. A link that does not fit makes HDF5 create an
//! indirect block, which the writer refuses.

use super::alloc::{Mem, Space};
use crate::checksum;
use crate::error::{Error, Result};
use crate::source::UNDEF;

/// `H5G_FHEAP_MAN_START_BLOCK_SIZE`.
const START_BLOCK: usize = 512;
/// `H5G_FHEAP_MAN_MAX_DIRECT_SIZE`, `H5G_FHEAP_MAX_MAN_SIZE`, `H5G_FHEAP_MAN_MAX_INDEX`,
/// `H5G_FHEAP_MAN_WIDTH`, `H5G_FHEAP_MAN_START_ROOT_ROWS`.
const MAX_DIRECT: u64 = 64 * 1024;
const MAX_MAN: usize = 4 * 1024;
const MAX_INDEX: u16 = 32;
const WIDTH: u16 = 4;
const START_ROOT_ROWS: u16 = 1;
/// Heap offsets take `(MAX_INDEX + 7) / 8` bytes, object lengths 2: heap IDs of 7 bytes.
const HEAP_OFF_SIZE: usize = 4;
const ID_LEN: usize = 1 + HEAP_OFF_SIZE + 2;
/// `H5HF_MAN_ABS_DIRECT_OVERHEAD`: signature, version, heap address, block offset, checksum.
const DBLOCK_OVERHEAD: usize = 4 + 1 + 8 + HEAP_OFF_SIZE + 4;
/// `H5HF_HEADER_SIZE`, `H5B2_HEADER_SIZE`, `H5FS_HEADER_SIZE` with 8-byte addresses and
/// lengths.
const HEAP_HDR_SIZE: u64 = 146;
const BT2_HDR_SIZE: u64 = 38;
const FS_HDR_SIZE: u64 = 82;
/// `H5G_NAME_BT2_NODE_SIZE`, `H5G_NAME_BT2_SPLIT_PERC`, `H5G_NAME_BT2_MERGE_PERC`.
const BT2_NODE: usize = 512;
const BT2_SPLIT: u8 = 100;
const BT2_MERGE: u8 = 40;
/// A name record: the name's hash and the link's heap ID.
const BT2_REC: usize = 4 + ID_LEN;
/// Records of a leaf: the node less its signature, version, type and checksum.
const BT2_LEAF_MAX: usize = (BT2_NODE - 10) / BT2_REC;
/// `H5HF_FSPACE_SHRINK`, `H5HF_FSPACE_EXPAND`; the heap's four section classes.
const FS_SHRINK: u16 = 80;
const FS_EXPAND: u16 = 120;
const FS_NCLASSES: u16 = 4;
/// Section info: the prefix (signature, version, header address, checksum), then per size
/// a count (1 byte for one section) and the size (3 bytes, for sizes up to `MAX_DIRECT`), and
/// per section its offset and class.
const SINFO_PREFIX: u64 = 4 + 1 + 8 + 4;
const SECT_LEN_SIZE: usize = 3;
/// `H5HF_FSPACE_SECT_SINGLE`.
const SECT_SINGLE: u8 = 0;

/// A link in the name index.
#[derive(Clone, Debug)]
struct Record {
    hash: u32,
    id: [u8; ID_LEN],
    name: Vec<u8>,
}

/// One group's dense storage.
#[derive(Clone, Debug)]
pub(crate) struct Dense {
    pub(crate) heap_addr: u64,
    pub(crate) bt2_addr: u64,
    /// The free-space manager's header (`UNDEF` before the first link and once the block is
    /// full).
    fs_addr: u64,
    /// The section info's real address, given at close.
    sinfo_addr: u64,
    leaf_addr: u64,
    /// The root direct block (empty until the first link), and its real address.
    block: Vec<u8>,
    block_addr: u64,
    /// The free section of the block: heap offset and size.
    section: Option<(usize, usize)>,
    /// `total_man_free`, `man_nobjs`.
    free: u64,
    nobjs: u64,
    records: Vec<Record>,
}

impl Dense {
    /// `H5G__dense_create`: the fractal heap's header, then the name index's header.
    pub(crate) fn create(space: &mut Space) -> Dense {
        let heap_addr = space.alloc(Mem::OHdr, HEAP_HDR_SIZE);
        let bt2_addr = space.alloc(Mem::BTree, BT2_HDR_SIZE);
        Dense {
            heap_addr,
            bt2_addr,
            fs_addr: UNDEF,
            sinfo_addr: UNDEF,
            leaf_addr: UNDEF,
            block: Vec::new(),
            block_addr: UNDEF,
            section: None,
            free: 0,
            nobjs: 0,
            records: Vec::new(),
        }
    }

    /// Whether the links named `names` fit in what the writer covers: the heap's first
    /// direct block and the B-tree's root leaf (hard links of one-byte name lengths).
    pub(crate) fn holds(names: &[&[u8]]) -> bool {
        let bytes: usize = names.iter().map(|n| 11 + n.len()).sum();
        names.len() <= BT2_LEAF_MAX
            && names.iter().all(|n| n.len() <= 255)
            && bytes <= START_BLOCK - DBLOCK_OVERHEAD
    }

    /// `H5G__dense_insert`: the encoded link message into the heap, then its record into the
    /// name index.
    pub(crate) fn insert(&mut self, space: &mut Space, name: &[u8], link: &[u8]) -> Result<()> {
        let size = link.len();
        let refuse = || {
            Err(Error::Unsupported(
                "a group whose links do not fit in its fractal heap's first block".into(),
            ))
        };
        if size > MAX_MAN {
            return refuse();
        }
        let hash = checksum::lookup3(name, 0);
        let pos = self
            .records
            .binary_search_by(|r| r.hash.cmp(&hash).then_with(|| r.name.as_slice().cmp(name)));
        let Err(pos) = pos else {
            return Err(Error::Write(format!(
                "a link named {:?} exists already",
                String::from_utf8_lossy(name)
            )));
        };
        if self.records.len() == BT2_LEAF_MAX {
            return refuse();
        }
        // H5HF__space_find, else H5HF__man_dblock_new for the root block.
        let (off, avail) = match self.section {
            Some((off, avail)) if avail >= size => (off, avail),
            _ if !self.block.is_empty() => return refuse(),
            _ => {
                let mut min = if size < START_BLOCK {
                    START_BLOCK
                } else {
                    1usize << (1 + size.ilog2())
                };
                if min < DBLOCK_OVERHEAD + size {
                    min *= 2;
                }
                if min != START_BLOCK {
                    return refuse();
                }
                self.block = vec![0u8; START_BLOCK];
                let avail = START_BLOCK - DBLOCK_OVERHEAD;
                self.free += avail as u64;
                (DBLOCK_OVERHEAD, avail)
            }
        };
        // H5HF__sect_single_reduce: the rest of the section goes back to the free-space
        // manager, which the first section creates (H5HF__space_add, H5FS_create).
        if avail == size {
            self.section = None;
        } else {
            self.section = Some((off + size, avail - size));
            if self.fs_addr == UNDEF {
                self.fs_addr = space.alloc(Mem::OHdr, FS_HDR_SIZE);
            }
        }
        self.block[off..off + size].copy_from_slice(link);
        self.nobjs += 1;
        self.free -= size as u64;
        let mut id = [0u8; ID_LEN];
        id[1..1 + HEAP_OFF_SIZE].copy_from_slice(&(off as u32).to_le_bytes());
        id[1 + HEAP_OFF_SIZE..].copy_from_slice(&(size as u16).to_le_bytes());
        // H5B2__insert: the root leaf is created by the first record.
        if self.leaf_addr == UNDEF {
            self.leaf_addr = space.alloc(Mem::BTree, BT2_NODE as u64);
        }
        self.records.insert(
            pos,
            Record {
                hash,
                id,
                name: name.to_vec(),
            },
        );
        // H5HF_close (H5HF__space_close): a free-space manager left without sections is
        // deleted, and its header's space freed (H5FS_delete).
        if self.section.is_none() && self.fs_addr != UNDEF {
            space.xfree(Mem::OHdr, self.fs_addr, FS_HDR_SIZE);
            self.fs_addr = UNDEF;
        }
        Ok(())
    }

    /// Real file space for the section info (`H5FS__cache_hdr_pre_serialize`).
    pub(crate) fn place_sections(&mut self, space: &mut Space) {
        if self.fs_addr != UNDEF && self.section.is_some() {
            self.sinfo_addr = space.alloc(Mem::OHdr, self.sinfo_size());
        }
    }

    /// Real file space for the root direct block (`H5HF__cache_dblock_pre_serialize`).
    pub(crate) fn place_block(&mut self, space: &mut Space) {
        if !self.block.is_empty() {
            self.block_addr = space.alloc(Mem::OHdr, START_BLOCK as u64);
        }
    }

    pub(crate) fn fs_addr(&self) -> u64 {
        self.fs_addr
    }

    /// `H5FS__sect_serialize_size` for one section.
    fn sinfo_size(&self) -> u64 {
        SINFO_PREFIX + 1 + SECT_LEN_SIZE as u64 + HEAP_OFF_SIZE as u64 + 1
    }

    /// The images of the heap header, the B-tree header and leaf, the free-space header and
    /// section info, and the direct block, at their addresses.
    pub(crate) fn serialize(&self) -> Vec<(u64, Vec<u8>)> {
        let mut out = Vec::new();
        let addr = |v: &mut Vec<u8>, a: u64| v.extend_from_slice(&a.to_le_bytes());
        let finish = |mut v: Vec<u8>| {
            let sum = checksum::metadata(&v);
            v.extend_from_slice(&sum.to_le_bytes());
            v
        };
        let table_addr = if self.block.is_empty() {
            UNDEF
        } else {
            self.block_addr
        };
        let man = if self.block.is_empty() {
            0
        } else {
            START_BLOCK as u64
        };
        // H5HF__cache_hdr_serialize.
        let mut h = Vec::with_capacity(HEAP_HDR_SIZE as usize);
        h.extend_from_slice(b"FRHP");
        h.push(0);
        h.extend_from_slice(&(ID_LEN as u16).to_le_bytes());
        h.extend_from_slice(&0u16.to_le_bytes()); // no I/O filters
        h.push(0x02); // direct blocks are checksummed
        h.extend_from_slice(&(MAX_MAN as u32).to_le_bytes());
        h.extend_from_slice(&0u64.to_le_bytes()); // next huge object ID
        addr(&mut h, UNDEF); // huge objects' B-tree
        h.extend_from_slice(&self.free.to_le_bytes());
        addr(&mut h, self.fs_addr);
        for v in [man, man, 0, self.nobjs, 0, 0, 0, 0] {
            h.extend_from_slice(&v.to_le_bytes());
        }
        h.extend_from_slice(&WIDTH.to_le_bytes());
        h.extend_from_slice(&(START_BLOCK as u64).to_le_bytes());
        h.extend_from_slice(&MAX_DIRECT.to_le_bytes());
        h.extend_from_slice(&MAX_INDEX.to_le_bytes());
        h.extend_from_slice(&START_ROOT_ROWS.to_le_bytes());
        addr(&mut h, table_addr);
        h.extend_from_slice(&0u16.to_le_bytes()); // the root is a direct block
        out.push((self.heap_addr, finish(h)));
        // H5B2__cache_hdr_serialize.
        let mut b = Vec::with_capacity(BT2_HDR_SIZE as usize);
        b.extend_from_slice(b"BTHD");
        b.push(0);
        b.push(5); // H5B2_GRP_DENSE_NAME_ID
        b.extend_from_slice(&(BT2_NODE as u32).to_le_bytes());
        b.extend_from_slice(&(BT2_REC as u16).to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes()); // depth
        b.push(BT2_SPLIT);
        b.push(BT2_MERGE);
        addr(&mut b, self.leaf_addr);
        b.extend_from_slice(&(self.records.len() as u16).to_le_bytes());
        b.extend_from_slice(&(self.records.len() as u64).to_le_bytes());
        out.push((self.bt2_addr, finish(b)));
        // H5B2__cache_leaf_serialize.
        if self.leaf_addr != UNDEF {
            let mut l = Vec::with_capacity(BT2_NODE);
            l.extend_from_slice(b"BTLF");
            l.push(0);
            l.push(5);
            for r in &self.records {
                l.extend_from_slice(&r.hash.to_le_bytes());
                l.extend_from_slice(&r.id);
            }
            let mut l = finish(l);
            l.resize(BT2_NODE, 0);
            out.push((self.leaf_addr, l));
        }
        if self.fs_addr != UNDEF {
            let (sect_off, sect_size) = self.section.unwrap_or((0, 0));
            let sinfo_size = self.sinfo_size();
            // H5FS__cache_hdr_serialize.
            let mut f = Vec::with_capacity(FS_HDR_SIZE as usize);
            f.extend_from_slice(b"FSHD");
            f.push(0);
            f.push(0); // H5FS_CLIENT_FHEAP_ID
            for v in [sect_size as u64, 1, 1, 0] {
                f.extend_from_slice(&v.to_le_bytes());
            }
            for v in [FS_NCLASSES, FS_SHRINK, FS_EXPAND, MAX_INDEX] {
                f.extend_from_slice(&v.to_le_bytes());
            }
            f.extend_from_slice(&MAX_DIRECT.to_le_bytes());
            addr(&mut f, self.sinfo_addr);
            f.extend_from_slice(&sinfo_size.to_le_bytes());
            f.extend_from_slice(&sinfo_size.to_le_bytes());
            out.push((self.fs_addr, finish(f)));
            // H5FS__cache_sinfo_serialize.
            let mut s = Vec::with_capacity(sinfo_size as usize);
            s.extend_from_slice(b"FSSE");
            s.push(0);
            addr(&mut s, self.fs_addr);
            s.push(1); // one section of this size
            s.extend_from_slice(&(sect_size as u64).to_le_bytes()[..SECT_LEN_SIZE]);
            s.extend_from_slice(&(sect_off as u64).to_le_bytes()[..HEAP_OFF_SIZE]);
            s.push(SECT_SINGLE);
            out.push((self.sinfo_addr, finish(s)));
        }
        // H5HF__cache_dblock_pre_serialize: the block's prefix, then a checksum of the whole
        // block with the checksum's own bytes zero.
        if !self.block.is_empty() {
            let mut d = self.block.clone();
            d[..4].copy_from_slice(b"FHDB");
            d[4] = 0;
            d[5..13].copy_from_slice(&self.heap_addr.to_le_bytes());
            d[13..13 + HEAP_OFF_SIZE].fill(0); // the root block's offset in the heap
            let c = 13 + HEAP_OFF_SIZE;
            d[c..c + 4].fill(0);
            let sum = checksum::metadata(&d);
            d[c..c + 4].copy_from_slice(&sum.to_le_bytes());
            out.push((self.block_addr, d));
        }
        out
    }
}
