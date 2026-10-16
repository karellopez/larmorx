// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! Global heap collections as HDF5 1.12.1 creates and fills them (`H5HG__create`,
//! `H5HG_insert`, `H5HG__alloc`, `H5HG_extend`, `src/H5HG.c`) and the file's list of
//! collections with free space (`H5F_cwfs_add`, `H5F_cwfs_find_free_heap`, `src/H5Fcwfs.c`).

use super::alloc::{Mem, Space};

/// `H5HG_MINSIZE`, `H5HG_MAXSIZE`, `H5F_NCWFS`.
const MINSIZE: usize = 4096;
const MAXSIZE: usize = 65536;
const NCWFS: usize = 16;
/// `H5HG_SIZEOF_HDR` and `H5HG_SIZEOF_OBJHDR` with 8-byte lengths.
const HDR: usize = 16;
const OBJHDR: usize = 16;

fn align8(x: usize) -> usize {
    x.div_ceil(8) * 8
}

/// One collection (`H5HG_heap_t`).
#[derive(Clone, Debug)]
pub(crate) struct Collection {
    pub(crate) addr: u64,
    pub(crate) image: Vec<u8>,
    /// Object 0 (the free space): its size and where it begins (`None` once exhausted).
    free_size: usize,
    free_begin: Option<usize>,
    nused: usize,
}

impl Collection {
    fn size(&self) -> usize {
        self.image.len()
    }

    fn write_free_header(&mut self) {
        if let Some(b) = self.free_begin {
            let s = self.free_size as u64;
            self.image[b..b + 8].fill(0);
            self.image[b + 8..b + 16].copy_from_slice(&s.to_le_bytes());
        }
    }
}

/// The file's global heap collections and its CWFS list.
#[derive(Clone, Debug, Default)]
pub(crate) struct GlobalHeap {
    pub(crate) collections: Vec<Collection>,
    /// Indices into `collections`, in CWFS order.
    cwfs: Vec<usize>,
}

/// A global heap ID: the collection's address and the object's index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct HeapId {
    pub(crate) addr: u64,
    pub(crate) index: u32,
}

impl GlobalHeap {
    /// `H5HG__create`.
    fn create(&mut self, space: &mut Space, size: usize) -> usize {
        let size = align8(size.max(MINSIZE));
        let addr = space.alloc(Mem::GHeap, size as u64);
        let mut image = vec![0u8; size];
        image[..4].copy_from_slice(b"GCOL");
        image[4] = 1;
        image[8..16].copy_from_slice(&(size as u64).to_le_bytes());
        let mut c = Collection {
            addr,
            image,
            free_size: size - HDR,
            free_begin: Some(HDR),
            nused: 1,
        };
        c.write_free_header();
        self.collections.push(c);
        let idx = self.collections.len() - 1;
        self.cwfs_add(idx);
        idx
    }

    /// `H5F_cwfs_add`.
    fn cwfs_add(&mut self, idx: usize) {
        if self.cwfs.len() == NCWFS {
            let free = self.collections[idx].free_size;
            for i in (0..NCWFS).rev() {
                if self.collections[self.cwfs[i]].free_size < free {
                    // memmove(cwfs + 1, cwfs, i): entries 0..i shift right, entry i is lost.
                    for j in (1..=i).rev() {
                        self.cwfs[j] = self.cwfs[j - 1];
                    }
                    self.cwfs[0] = idx;
                    break;
                }
            }
        } else {
            self.cwfs.insert(0, idx);
        }
    }

    /// `H5F_cwfs_find_free_heap`.
    fn find_free(&mut self, space: &mut Space, need: usize) -> Option<usize> {
        let mut found: Option<usize> = None;
        for (pos, &c) in self.cwfs.iter().enumerate() {
            if self.collections[c].free_size >= need {
                found = Some(pos);
                break;
            }
        }
        if found.is_none() {
            for pos in 0..self.cwfs.len() {
                let c = self.cwfs[pos];
                let (size, free, addr) = {
                    let col = &self.collections[c];
                    (col.size(), col.free_size, col.addr)
                };
                let new_need = (need - free).max(size);
                if size + new_need <= MAXSIZE
                    && space.try_extend(Mem::GHeap, addr, size as u64, new_need as u64)
                {
                    self.extend(c, new_need);
                    found = Some(pos);
                    break;
                }
            }
        }
        let pos = found?;
        let c = self.cwfs[pos];
        if pos > 0 {
            self.cwfs.swap(pos, pos - 1);
        }
        Some(c)
    }

    /// `H5HG_extend`.
    fn extend(&mut self, c: usize, need: usize) {
        let col = &mut self.collections[c];
        let old_size = col.image.len();
        col.image.resize(old_size + need, 0);
        let new_size = col.image.len() as u64;
        col.image[8..16].copy_from_slice(&new_size.to_le_bytes());
        col.free_size += need;
        if col.free_begin.is_none() {
            col.free_begin = Some(old_size);
        }
        col.write_free_header();
    }

    /// `H5HG_insert`: stores `data` as a new object.
    pub(crate) fn insert(&mut self, space: &mut Space, data: &[u8]) -> HeapId {
        let need = OBJHDR + align8(data.len());
        let c = match self.find_free(space, need) {
            Some(c) => c,
            None => self.create(space, need + HDR),
        };
        let col = &mut self.collections[c];
        // H5HG__alloc
        let idx = col.nused;
        col.nused += 1;
        let begin = col.free_begin.expect("free space");
        col.image[begin..begin + 2].copy_from_slice(&(idx as u16).to_le_bytes());
        col.image[begin + 2..begin + 8].fill(0);
        col.image[begin + 8..begin + 16].copy_from_slice(&(data.len() as u64).to_le_bytes());
        if need == col.free_size {
            col.free_size = 0;
            col.free_begin = None;
        } else if col.free_size - need >= OBJHDR {
            col.free_size -= need;
            col.free_begin = Some(begin + need);
            col.write_free_header();
        } else {
            col.free_size -= need;
            col.free_begin = Some(begin + need);
        }
        col.image[begin + OBJHDR..begin + OBJHDR + data.len()].copy_from_slice(data);
        HeapId {
            addr: col.addr,
            index: idx as u32,
        }
    }
}
