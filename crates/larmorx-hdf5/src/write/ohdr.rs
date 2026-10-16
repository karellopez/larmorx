// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! Version-2 object headers as HDF5 1.12.1 builds them in memory and writes them: creation
//! (`H5O_create`, `H5O_apply_ohdr`, `src/H5Oint.c`), room for messages (`H5O__alloc`,
//! `H5O__alloc_null`, `H5O__alloc_extend_chunk`, `H5O__alloc_new_chunk`, `H5O__alloc_chunk`,
//! `H5O__add_gap`, `H5O__eliminate_gap`, `src/H5Oalloc.c`), and serialization
//! (`H5O__cache_serialize`, `H5O__chunk_serialize`, `H5O_msg_flush`, `src/H5Ocache.c`,
//! `src/H5Omessage.c`). Chunk images are kept as HDF5 keeps them, so the bytes it would leave
//! in null messages and gaps are the same.

use super::alloc::{Mem, Space};
use super::encode::Native;
use crate::checksum;

/// Size of a message header in a version-2 object header without creation order tracking.
const MSGHDR: usize = 4;
const CHKSUM: usize = 4;
/// Signature and checksum of a continuation chunk.
const CHKHDR: usize = 8;
/// `H5O_MIN_SIZE`.
const MIN_SIZE: usize = 22;

/// `H5O_HDR_STORE_TIMES`.
pub(crate) const STORE_TIMES: u8 = 0x20;

/// One message (`H5O_mesg_t`).
#[derive(Clone, Debug)]
pub(crate) struct Mesg {
    pub(crate) native: Native,
    /// Offset of the message's data in its chunk's image.
    pub(crate) raw: usize,
    pub(crate) raw_size: usize,
    pub(crate) chunkno: usize,
    pub(crate) flags: u8,
}

impl Mesg {
    fn is_null(&self) -> bool {
        matches!(self.native, Native::Null)
    }
}

/// One chunk (`H5O_chunk_t`): chunk 0's image holds the prefix too.
#[derive(Clone, Debug)]
pub(crate) struct Chunk {
    pub(crate) addr: u64,
    pub(crate) size: usize,
    pub(crate) gap: usize,
    pub(crate) image: Vec<u8>,
}

/// A version-2 object header (`H5O_t`).
#[derive(Clone, Debug)]
pub(crate) struct ObjHeader {
    pub(crate) flags: u8,
    pub(crate) time: u32,
    pub(crate) chunks: Vec<Chunk>,
    pub(crate) mesgs: Vec<Mesg>,
}

/// Where a moved message's space came from (`H5O_msg_alloc_info_t`).
#[derive(Clone, Copy, Debug)]
struct FoundMsg {
    msgno: usize,
    chunkno: usize,
    gap_size: usize,
    null_size: usize,
    total_size: usize,
    null_msgno: usize,
}

impl ObjHeader {
    /// `H5O_SIZEOF_HDR`: the prefix and chunk 0's checksum.
    fn sizeof_hdr(&self) -> usize {
        4 + 1
            + 1
            + if self.flags & STORE_TIMES != 0 { 16 } else { 0 }
            + (1usize << (self.flags & 0x03))
            + CHKSUM
    }

    /// `H5O_create` with `size_hint` bytes for messages: the header is allocated at once.
    pub(crate) fn create(space: &mut Space, size_hint: usize, time: u32) -> ObjHeader {
        let size_hint = size_hint.max(MIN_SIZE);
        let mut flags = STORE_TIMES;
        if size_hint > 4_294_967_295 {
            flags |= 0x03;
        } else if size_hint > 65535 {
            flags |= 0x02;
        } else if size_hint > 255 {
            flags |= 0x01;
        }
        let mut oh = ObjHeader {
            flags,
            time,
            chunks: Vec::new(),
            mesgs: Vec::new(),
        };
        let oh_size = oh.sizeof_hdr() + size_hint;
        let addr = space.alloc(Mem::OHdr, oh_size as u64);
        let mut image = vec![0u8; oh_size];
        image[..4].copy_from_slice(b"OHDR");
        oh.chunks.push(Chunk {
            addr,
            size: oh_size,
            gap: 0,
            image,
        });
        let raw = oh.sizeof_hdr() - CHKSUM + MSGHDR;
        oh.mesgs.push(Mesg {
            native: Native::Null,
            raw,
            raw_size: size_hint - MSGHDR,
            chunkno: 0,
            flags: 0,
        });
        oh
    }

    pub(crate) fn addr(&self) -> u64 {
        self.chunks[0].addr
    }

    /// `H5O_msg_append_oh`: room for the message, then the message itself. Returns its index.
    pub(crate) fn append(&mut self, space: &mut Space, native: Native, flags: u8) -> usize {
        let size = native.raw_size();
        let idx = self.alloc(space, size);
        let m = &mut self.mesgs[idx];
        m.native = native;
        m.flags = flags;
        idx
    }

    /// Replaces a message of the same encoded size (`H5O_msg_write`).
    pub(crate) fn rewrite(&mut self, idx: usize, native: Native) {
        debug_assert_eq!(native.raw_size(), self.mesgs[idx].native.raw_size());
        self.mesgs[idx].native = native;
    }

    /// `H5O__alloc_find_best_null`.
    fn find_best_null(&self, size: usize) -> Option<usize> {
        let mut found: Option<usize> = None;
        for (idx, m) in self.mesgs.iter().enumerate() {
            if !m.is_null() {
                continue;
            }
            if m.raw_size == size {
                match found {
                    None => found = Some(idx),
                    Some(f) => {
                        if m.chunkno < self.mesgs[f].chunkno {
                            found = Some(idx);
                            if m.chunkno == 0 {
                                break;
                            }
                        }
                    }
                }
            } else if m.raw_size > size {
                match found {
                    None => found = Some(idx),
                    Some(f) => {
                        let fm = &self.mesgs[f];
                        if m.raw_size < fm.raw_size
                            || (m.raw_size == fm.raw_size && m.chunkno < fm.chunkno)
                        {
                            found = Some(idx);
                        }
                    }
                }
            }
        }
        found
    }

    /// `H5O__alloc`.
    fn alloc(&mut self, space: &mut Space, raw_size: usize) -> usize {
        let mut idx = self.find_best_null(raw_size);
        if idx.is_none() {
            for chunkno in 0..self.chunks.len() {
                if let Some(i) = self.alloc_extend_chunk(space, chunkno, raw_size) {
                    idx = Some(i);
                    break;
                }
            }
        }
        let idx = match idx {
            Some(i) => i,
            None => self.alloc_new_chunk(space, raw_size),
        };
        self.alloc_null(idx, raw_size);
        idx
    }

    /// `H5O__alloc_null`: takes `new_size` bytes of null message `null_idx` (the caller
    /// sets the message's content).
    fn alloc_null(&mut self, null_idx: usize, new_size: usize) {
        let (raw, raw_size, chunkno) = {
            let m = &self.mesgs[null_idx];
            (m.raw, m.raw_size, m.chunkno)
        };
        if raw_size > new_size {
            if raw_size - new_size < MSGHDR {
                let gap_size = raw_size - new_size;
                self.mesgs[null_idx].raw_size = new_size;
                self.add_gap(chunkno, null_idx, raw + new_size, gap_size);
            } else {
                let new_mesg_size = new_size + MSGHDR;
                self.mesgs.push(Mesg {
                    native: Native::Null,
                    raw: raw + new_mesg_size,
                    raw_size: raw_size - new_mesg_size,
                    chunkno,
                    flags: 0,
                });
                let null_idx2 = self.mesgs.len() - 1;
                let gap = self.chunks[chunkno].gap;
                if gap > 0 {
                    let gap_loc = self.chunks[chunkno].size - (CHKSUM + gap);
                    self.eliminate_gap(null_idx2, gap_loc, gap);
                }
                self.mesgs[null_idx].raw_size = new_size;
            }
        }
        // The caller gives the message its content; mark it as no longer null.
        self.mesgs[null_idx].native = Native::Placeholder;
    }

    /// `H5O__add_gap`.
    fn add_gap(&mut self, chunkno: usize, idx: usize, new_gap_loc: usize, mut new_gap_size: usize) {
        let null = self
            .mesgs
            .iter()
            .enumerate()
            .find(|(u, m)| m.is_null() && m.chunkno == chunkno && *u != idx)
            .map(|(u, _)| u);
        if let Some(u) = null {
            self.eliminate_gap(u, new_gap_loc, new_gap_size);
            return;
        }
        for m in &mut self.mesgs {
            if m.chunkno == chunkno && m.raw > new_gap_loc {
                m.raw -= new_gap_size;
            }
        }
        let chunk = &mut self.chunks[chunkno];
        let end = chunk.size - CHKSUM;
        chunk
            .image
            .copy_within(new_gap_loc + new_gap_size..end, new_gap_loc);
        new_gap_size += chunk.gap;
        if new_gap_size >= MSGHDR {
            let raw_size = new_gap_size - MSGHDR;
            let raw = chunk.size - (CHKSUM + raw_size);
            chunk.image[raw..raw + raw_size].fill(0);
            chunk.gap = 0;
            self.mesgs.push(Mesg {
                native: Native::Null,
                raw,
                raw_size,
                chunkno,
                flags: 0,
            });
        } else {
            chunk.gap = new_gap_size;
        }
    }

    /// `H5O__eliminate_gap`: null message `idx` absorbs the gap.
    fn eliminate_gap(&mut self, idx: usize, gap_loc: usize, gap_size: usize) {
        let (raw, raw_size, chunkno) = {
            let m = &self.mesgs[idx];
            (m.raw, m.raw_size, m.chunkno)
        };
        let null_before_gap = raw < gap_loc;
        let (move_start, move_end) = if null_before_gap {
            (raw + raw_size, gap_loc)
        } else {
            (gap_loc + gap_size, raw - MSGHDR)
        };
        let mut new_raw = raw;
        if move_end > move_start {
            for (u, m) in self.mesgs.iter_mut().enumerate() {
                let start = m.raw - MSGHDR;
                if u != idx && m.chunkno == chunkno && start >= move_start && start < move_end {
                    if null_before_gap {
                        m.raw += gap_size;
                    } else {
                        m.raw -= gap_size;
                    }
                }
            }
            let image = &mut self.chunks[chunkno].image;
            if null_before_gap {
                image.copy_within(move_start..move_end, move_start + gap_size);
            } else {
                image.copy_within(move_start..move_end, move_start - gap_size);
                new_raw = raw - gap_size;
            }
        } else if move_end == move_start && !null_before_gap {
            let image = &mut self.chunks[chunkno].image;
            image.copy_within(
                move_start..move_start + raw_size + MSGHDR,
                move_start - gap_size,
            );
            new_raw = raw - gap_size;
        }
        let image = &mut self.chunks[chunkno].image;
        image[new_raw + raw_size..new_raw + raw_size + gap_size].fill(0);
        let m = &mut self.mesgs[idx];
        m.raw = new_raw;
        m.raw_size += gap_size;
        self.chunks[chunkno].gap = 0;
    }

    /// `H5O__alloc_extend_chunk`: grows chunk `chunkno` in the file, if the file allows.
    fn alloc_extend_chunk(
        &mut self,
        space: &mut Space,
        chunkno: usize,
        size: usize,
    ) -> Option<usize> {
        let aligned = size;
        let (csize, gap) = (self.chunks[chunkno].size, self.chunks[chunkno].gap);
        let extend_msg = self.mesgs.iter().position(|m| {
            m.chunkno == chunkno && m.is_null() && m.raw + m.raw_size == csize - (gap + CHKSUM)
        });
        let delta = match extend_msg {
            Some(i) => aligned - self.mesgs[i].raw_size,
            None => (aligned + MSGHDR) - gap,
        };
        let mut extra_prfx = 0usize;
        let mut new_size_flags = 0u8;
        let mut adjust = false;
        if chunkno == 0 {
            let chunk0_size = (self.chunks[0].size - self.sizeof_hdr()) as u64;
            let orig = 1usize << (self.flags & 0x03);
            let grown = chunk0_size + delta as u64;
            if orig < 8 && grown > 4_294_967_295 {
                extra_prfx = 8 - orig;
                new_size_flags = 0x03;
                adjust = true;
            } else if orig < 4 && grown > 65535 {
                extra_prfx = 4 - orig;
                new_size_flags = 0x02;
                adjust = true;
            } else if orig < 2 && grown > 255 {
                extra_prfx = 2 - orig;
                new_size_flags = 0x01;
                adjust = true;
            }
        }
        let addr = self.chunks[chunkno].addr;
        if !space.try_extend(Mem::OHdr, addr, csize as u64, (delta + extra_prfx) as u64) {
            return None;
        }
        if adjust {
            self.flags = (self.flags & !0x03) | new_size_flags;
        }
        let ext = match extend_msg {
            Some(i) => {
                self.mesgs[i].raw_size += delta;
                i
            }
            None => {
                self.mesgs.push(Mesg {
                    native: Native::Null,
                    raw: (csize - (CHKSUM + gap)) + MSGHDR,
                    raw_size: (delta + gap) - MSGHDR,
                    chunkno,
                    flags: 0,
                });
                self.mesgs.len() - 1
            }
        };
        let old_size = csize;
        let new_size = csize + delta + extra_prfx;
        {
            let chunk = &mut self.chunks[chunkno];
            chunk.size = new_size;
            chunk.image.resize(new_size, 0);
            chunk.gap = 0;
        }
        if adjust {
            let hdr = self.sizeof_hdr();
            let image = &mut self.chunks[0].image;
            let src = hdr - CHKSUM - extra_prfx;
            let len = old_size - hdr + extra_prfx;
            image.copy_within(src..src + len, hdr - CHKSUM);
        }
        for m in &mut self.mesgs {
            if m.chunkno == chunkno {
                m.raw += extra_prfx;
            }
        }
        if chunkno > 0 {
            for m in &mut self.mesgs {
                if let Native::Cont {
                    chunkno: c, size, ..
                } = &mut m.native
                    && *c == chunkno
                {
                    *size = new_size as u64;
                }
            }
        }
        Some(ext)
    }

    /// `H5O__alloc_find_best_nonnull`: the message to move to a new chunk to make room for a
    /// continuation message; adds what must move to `size`.
    fn find_best_nonnull(&self, size: &mut usize) -> Option<FoundMsg> {
        let cont_size = 16usize;
        let mut found: Option<FoundMsg> = None;
        let mut found_is_attr = false;
        let mut multi_size = 0usize;
        let nchunks = self.chunks.len();
        for (u, m) in self.mesgs.iter().enumerate() {
            if matches!(m.native, Native::Cont { .. }) {
                continue;
            }
            let ck = &self.chunks[m.chunkno];
            let end_chunk_data = ck.size - (CHKSUM + ck.gap);
            let end_msg = m.raw + m.raw_size;
            let (mut gap_size, mut null_size, mut null_msgno) = (0, 0, 0);
            if end_msg == end_chunk_data {
                gap_size = ck.gap;
            } else if let Some((v, t)) =
                self.mesgs.iter().enumerate().find(|(_, t)| {
                    t.is_null() && t.chunkno == m.chunkno && t.raw - MSGHDR == end_msg
                })
            {
                null_msgno = v;
                null_size = MSGHDR + t.raw_size;
            }
            let total_size = m.raw_size + gap_size + null_size;
            if total_size >= cont_size {
                let is_attr = false;
                let better = match &found {
                    None => true,
                    Some(f) => {
                        (found_is_attr && !is_attr)
                            || total_size < f.total_size
                            || (total_size == f.total_size && m.chunkno < f.chunkno)
                    }
                };
                if better {
                    found = Some(FoundMsg {
                        msgno: u,
                        chunkno: m.chunkno,
                        gap_size,
                        null_size,
                        total_size,
                        null_msgno,
                    });
                    found_is_attr = is_attr;
                }
            } else if found.is_none() && m.chunkno == nchunks - 1 {
                multi_size += m.raw_size + MSGHDR;
            }
        }
        match &found {
            None => *size += multi_size,
            Some(f) => *size += MSGHDR + self.mesgs[f.msgno].raw_size,
        }
        found
    }

    /// `H5O__alloc_new_chunk`.
    fn alloc_new_chunk(&mut self, space: &mut Space, size: usize) -> usize {
        let mut size = size;
        let cont_size = 16usize;
        let idx = self.find_best_null(cont_size);
        let found = if idx.is_none() {
            self.find_best_nonnull(&mut size)
        } else {
            None
        };
        self.alloc_chunk(space, size, idx, found)
    }

    /// `H5O__alloc_chunk`: a new continuation chunk, the continuation message pointing to it,
    /// and the null message of the new chunk's free space (whose index is returned).
    fn alloc_chunk(
        &mut self,
        space: &mut Space,
        size: usize,
        found_null: Option<usize>,
        found_msg: Option<FoundMsg>,
    ) -> usize {
        let mut size = (size + MSGHDR).max(MIN_SIZE);
        size += CHKHDR;
        let addr = space.alloc(Mem::OHdr, size as u64);
        let chunkno = self.chunks.len();
        let mut image = vec![0u8; size];
        image[..4].copy_from_slice(b"OCHK");
        let mut p = 4usize;
        self.chunks.push(Chunk {
            addr,
            size,
            gap: 0,
            image,
        });
        let mut found_null = found_null;
        if found_null.is_none() {
            match found_msg {
                None => {
                    // Move every message of the last chunk into the new one.
                    let last = chunkno - 1;
                    // HDF5's loop goes on to the next index after removing a null message,
                    // so the message moved into its place is not looked at.
                    let mut u = 0;
                    while u < self.mesgs.len() {
                        if self.mesgs[u].chunkno == last {
                            if self.mesgs[u].is_null() {
                                self.mesgs.remove(u);
                                u += 1;
                                continue;
                            }
                            let (raw, raw_size) = (self.mesgs[u].raw, self.mesgs[u].raw_size);
                            let bytes =
                                self.chunks[last].image[raw - MSGHDR..raw + raw_size].to_vec();
                            self.chunks[chunkno].image[p..p + bytes.len()].copy_from_slice(&bytes);
                            self.mesgs[u].chunkno = chunkno;
                            self.mesgs[u].raw = p + MSGHDR;
                            p += MSGHDR + raw_size;
                            size -= MSGHDR + raw_size;
                        }
                        u += 1;
                    }
                    let start = if last == 0 { self.sizeof_hdr() } else { CHKHDR };
                    self.mesgs.push(Mesg {
                        native: Native::Null,
                        raw: start - CHKSUM + MSGHDR,
                        raw_size: self.chunks[last].size - start - MSGHDR,
                        chunkno: last,
                        flags: 0,
                    });
                    found_null = Some(self.mesgs.len() - 1);
                    self.chunks[last].gap = 0;
                }
                Some(f) => {
                    let (raw, raw_size, mchunk) = {
                        let m = &self.mesgs[f.msgno];
                        (m.raw, m.raw_size, m.chunkno)
                    };
                    self.mesgs.push(Mesg {
                        native: Native::Null,
                        raw,
                        raw_size,
                        chunkno: mchunk,
                        flags: 0,
                    });
                    let mut null_idx = self.mesgs.len() - 1;
                    let bytes = self.chunks[mchunk].image[raw - MSGHDR..raw + raw_size].to_vec();
                    self.chunks[chunkno].image[p..p + bytes.len()].copy_from_slice(&bytes);
                    self.mesgs[f.msgno].raw = p + MSGHDR;
                    self.mesgs[f.msgno].chunkno = chunkno;
                    p += MSGHDR + raw_size;
                    size -= MSGHDR + raw_size;
                    if f.gap_size > 0 {
                        self.mesgs[null_idx].raw_size += f.gap_size;
                        self.chunks[mchunk].gap = 0;
                    } else if f.null_size > 0 {
                        self.mesgs[null_idx].raw_size += f.null_size;
                        self.mesgs.remove(f.null_msgno);
                        if f.null_msgno < null_idx {
                            null_idx -= 1;
                        }
                    }
                    found_null = Some(null_idx);
                }
            }
        }
        // The null message of the rest of the new chunk.
        self.mesgs.push(Mesg {
            native: Native::Null,
            raw: p + MSGHDR,
            raw_size: size - (CHKHDR + MSGHDR),
            chunkno,
            flags: 0,
        });
        let idx = self.mesgs.len() - 1;
        // The continuation message, in the null message found or made.
        let fnull = found_null.unwrap();
        self.alloc_null(fnull, 16);
        self.mesgs[fnull].native = Native::Cont {
            addr,
            size: self.chunks[chunkno].size as u64,
            chunkno,
        };
        idx
    }

    /// The bytes of every chunk at its address (`H5O__cache_serialize`,
    /// `H5O__cache_chk_serialize`).
    pub(crate) fn serialize(&self) -> Vec<(u64, Vec<u8>)> {
        let mut out = Vec::with_capacity(self.chunks.len());
        for (chunkno, chunk) in self.chunks.iter().enumerate() {
            let mut image = chunk.image.clone();
            if chunkno == 0 {
                let mut p = 4;
                image[p] = 2;
                p += 1;
                image[p] = self.flags;
                p += 1;
                if self.flags & STORE_TIMES != 0 {
                    for _ in 0..4 {
                        image[p..p + 4].copy_from_slice(&self.time.to_le_bytes());
                        p += 4;
                    }
                }
                let chunk0 = (chunk.size - self.sizeof_hdr()) as u64;
                let n = 1usize << (self.flags & 0x03);
                image[p..p + n].copy_from_slice(&chunk0.to_le_bytes()[..n]);
            }
            for m in self.mesgs.iter().filter(|m| m.chunkno == chunkno) {
                let h = m.raw - MSGHDR;
                image[h] = m.native.type_id();
                image[h + 1..h + 3].copy_from_slice(&(m.raw_size as u16).to_le_bytes());
                image[h + 3] = m.flags;
                if !m.is_null() {
                    let bytes = m.native.encode();
                    image[m.raw..m.raw + bytes.len()].copy_from_slice(&bytes);
                }
            }
            if chunk.gap > 0 {
                let end = chunk.size - CHKSUM;
                image[end - chunk.gap..end].fill(0);
            }
            let sum = checksum::metadata(&image[..chunk.size - CHKSUM]);
            let n = image.len();
            image[n - CHKSUM..].copy_from_slice(&sum.to_le_bytes());
            out.push((chunk.addr, image));
        }
        out
    }
}
