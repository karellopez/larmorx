// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! File space allocation as HDF5 1.12.1 does it for a file created with the default
//! properties and the `sec2` driver (`src/H5MF.c`, `src/H5MFaggr.c`, `src/H5MFsection.c`,
//! `src/H5FSsection.c`, `src/H5FDspace.c`): the free-space strategy `FSM_AGGR` with
//! non-persistent free-space managers, no alignment, a threshold of 1 byte, and two block
//! aggregators of 2048 bytes, one for metadata and one for "small" raw data (global heaps count
//! as raw data). The `sec2` driver's free-list map is the "dichotomy" one: every metadata type
//! shares one free-space manager, raw data and global heaps the other.

use std::collections::BTreeMap;

use crate::source::UNDEF;

/// `H5FD_mem_t`: what a block of file space holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mem {
    /// `H5FD_MEM_DEFAULT`: the metadata aggregator's own type.
    Default,
    Super,
    BTree,
    Draw,
    GHeap,

    OHdr,
}

impl Mem {
    /// Raw data and global heaps go to the "small data" aggregator and the raw free-space
    /// manager; everything else is metadata.
    fn raw(self) -> bool {
        matches!(self, Mem::Draw | Mem::GHeap)
    }
}

/// `H5F_blk_aggr_t`.
#[derive(Clone, Copy, Debug)]
struct Aggr {
    addr: u64,
    size: u64,
    tot_size: u64,
    alloc_size: u64,
}

impl Aggr {
    fn new() -> Aggr {
        Aggr {
            addr: 0,
            size: 0,
            tot_size: 0,
            alloc_size: 2048,
        }
    }
}

/// How a section can shrink (`H5MF_shrink_type_t`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shrink {
    Eoa,
    AggrAbsorbSect,
    SectAbsorbAggr,
}

/// A free-space manager of simple sections: addresses to sizes (the merge list), searched by
/// size for allocations (best fit, lowest address among equal sizes).
#[derive(Clone, Debug, Default)]
struct FreeSpace {
    sections: BTreeMap<u64, u64>,
}

impl FreeSpace {
    /// `H5FS__sect_find_node`: the smallest section of at least `size` bytes, the lowest
    /// address among equal sizes.
    fn find(&self, size: u64) -> Option<(u64, u64)> {
        let mut best: Option<(u64, u64)> = None;
        for (&addr, &s) in &self.sections {
            if s >= size && best.is_none_or(|(_, bs)| s < bs) {
                best = Some((addr, s));
            }
        }
        best
    }
}

/// The file's space: its end of allocated address space (EOA), the two aggregators and the
/// two free-space managers.
#[derive(Clone, Debug)]
pub(crate) struct Space {
    pub(crate) eoa: u64,
    meta: Aggr,
    sdata: Aggr,
    /// [metadata, raw data]; `None` until started (`H5MF__start_fstype`).
    fs: [Option<FreeSpace>; 2],
}

/// `EXTEND_THRESHOLD` of `H5MFaggr.c`.
const EXTEND_THRESHOLD: f32 = 0.10;

impl Space {
    pub(crate) fn new() -> Space {
        Space {
            eoa: 0,
            meta: Aggr::new(),
            sdata: Aggr::new(),
            fs: [None, None],
        }
    }

    fn fs_index(t: Mem) -> usize {
        usize::from(t.raw())
    }

    /// `H5F__alloc` (`H5FD__extend`): space at the end of the file.
    fn vfd_alloc(&mut self, size: u64) -> u64 {
        let addr = self.eoa;
        self.eoa += size;
        addr
    }

    /// `H5F__try_extend` (`H5FD_try_extend`): a block ending at the EOA grows with it.
    fn vfd_try_extend(&mut self, blk_end: u64, extra: u64) -> bool {
        if blk_end == self.eoa {
            self.eoa += extra;
            true
        } else {
            false
        }
    }

    /// `H5F__free` (`H5FD__free_real` without a driver callback): a block at the EOA shrinks
    /// it; any other is left as it is.
    fn vfd_free(&mut self, addr: u64, size: u64) {
        if self.eoa == addr.wrapping_add(size) {
            self.eoa = addr;
        }
    }

    fn aggr(&mut self, raw: bool) -> &mut Aggr {
        if raw { &mut self.sdata } else { &mut self.meta }
    }

    /// `H5MF_alloc`.
    pub(crate) fn alloc(&mut self, t: Mem, size: u64) -> u64 {
        let fi = Self::fs_index(t);
        if let Some(fs) = &self.fs[fi]
            && let Some((addr, ssize)) = fs.find(size)
        {
            // H5MF__find_sect: take the front of the section; re-add the rest.
            self.fs[fi].as_mut().unwrap().sections.remove(&addr);
            if ssize > size {
                self.add_sect(t, addr + size, ssize - size);
            }
            return addr;
        }
        self.aggr_vfd_alloc(t, size)
    }

    /// `H5MF_aggr_vfd_alloc`.
    fn aggr_vfd_alloc(&mut self, t: Mem, size: u64) -> u64 {
        if t.raw() {
            self.aggr_alloc(true, size)
        } else {
            self.aggr_alloc(false, size)
        }
    }

    /// `H5MF__aggr_alloc` (no alignment, so no fragments).
    fn aggr_alloc(&mut self, raw: bool, size: u64) -> u64 {
        let eoa = self.eoa;
        let alloc_type = if raw { Mem::Draw } else { Mem::Default };
        let other_type = if raw { Mem::Default } else { Mem::Draw };
        let a = *self.aggr(raw);
        if size > a.size {
            if size >= a.alloc_size {
                let ext_size = size;
                if a.addr > 0 && self.vfd_try_extend(a.addr.wrapping_add(a.size), ext_size) {
                    let ag = self.aggr(raw);
                    let ret = ag.addr;
                    ag.addr += ext_size;
                    ag.tot_size += ext_size;
                    return ret;
                }
                self.maybe_release_other(raw, eoa, other_type);
                return self.vfd_alloc(size);
            }
            let ext_size = a.alloc_size;
            if a.addr > 0 && self.vfd_try_extend(a.addr.wrapping_add(a.size), ext_size) {
                let ag = self.aggr(raw);
                ag.size += ext_size;
                ag.tot_size += ext_size;
            } else {
                self.maybe_release_other(raw, eoa, other_type);
                let new_space = self.vfd_alloc(a.alloc_size);
                // Return the unused part of the old block.
                let old = *self.aggr(raw);
                if old.size > 0 {
                    self.xfree(alloc_type, old.addr, old.size);
                }
                let ag = self.aggr(raw);
                ag.addr = new_space;
                ag.size = ag.alloc_size;
                ag.tot_size = ag.alloc_size;
            }
            let ag = self.aggr(raw);
            let ret = ag.addr;
            ag.size -= size;
            ag.addr += size;
            ret
        } else {
            let ag = self.aggr(raw);
            let ret = ag.addr;
            ag.size -= size;
            ag.addr += size;
            ret
        }
    }

    /// Releases the other aggregator when it is at the EOA, has allocated more than one block
    /// and has at least a block unused (`H5MF__aggr_alloc`, `H5MF__aggr_free`).
    fn maybe_release_other(&mut self, raw: bool, eoa: u64, _other_type: Mem) {
        let o = *self.aggr(!raw);
        if o.size > 0
            && o.addr.wrapping_add(o.size) == eoa
            && o.tot_size > o.size
            && (o.tot_size - o.size) >= o.alloc_size
        {
            self.vfd_free(o.addr, o.size);
            let ag = self.aggr(!raw);
            ag.tot_size = 0;
            ag.addr = UNDEF;
            ag.size = 0;
        }
    }

    /// `H5MF_xfree`.
    pub(crate) fn xfree(&mut self, t: Mem, addr: u64, size: u64) {
        if addr == UNDEF || size == 0 {
            return;
        }
        let fi = Self::fs_index(t);
        if self.fs[fi].is_none() {
            if self.try_shrink(t, addr, size) {
                return;
            }
            self.fs[fi] = Some(FreeSpace::default());
        }
        self.add_sect(t, addr, size);
    }

    /// `H5MF_try_shrink`: the block shrinks the EOA, or the aggregator absorbs it.
    fn try_shrink(&mut self, t: Mem, addr: u64, size: u64) -> bool {
        let mut sect = (addr, size);
        match self.can_shrink(t, sect, false) {
            Some((shrink, raw)) => {
                self.shrink(shrink, raw, &mut sect, false);
                true
            }
            None => false,
        }
    }

    /// `H5MF__sect_simple_can_shrink`: the kind of shrink and the aggregator involved.
    fn can_shrink(&self, t: Mem, sect: (u64, u64), eoa_only: bool) -> Option<(Shrink, bool)> {
        let end = sect.0 + sect.1;
        if end == self.eoa {
            return Some((Shrink::Eoa, false));
        }
        if eoa_only {
            return None;
        }
        // The dichotomy map: metadata merges with the metadata aggregator, raw data and
        // global heaps with the small-data aggregator.
        let raw = t.raw();
        let ag = if raw { &self.sdata } else { &self.meta };
        if end == ag.addr || ag.addr.wrapping_add(ag.size) == sect.0 {
            let shrink = if ag.size + sect.1 >= ag.alloc_size {
                Shrink::SectAbsorbAggr
            } else {
                Shrink::AggrAbsorbSect
            };
            return Some((shrink, raw));
        }
        None
    }

    /// `H5MF__sect_simple_shrink` with `H5MF__aggr_absorb`; returns whether the section
    /// survives (it absorbed the aggregator).
    fn shrink(
        &mut self,
        shrink: Shrink,
        raw: bool,
        sect: &mut (u64, u64),
        allow_sect_absorb: bool,
    ) -> bool {
        match shrink {
            Shrink::Eoa => {
                self.vfd_free(sect.0, sect.1);
                false
            }
            _ => {
                let ag = self.aggr(raw);
                if ag.size + sect.1 >= ag.alloc_size && allow_sect_absorb {
                    if sect.0 + sect.1 == ag.addr {
                        sect.1 += ag.size;
                    } else {
                        sect.0 -= ag.size;
                        sect.1 += ag.size;
                    }
                    ag.tot_size = 0;
                    ag.addr = 0;
                    ag.size = 0;
                } else if sect.0 + sect.1 == ag.addr {
                    ag.addr -= sect.1;
                    ag.size += sect.1;
                    ag.tot_size -= ag.tot_size.min(sect.1);
                } else {
                    ag.size += sect.1;
                }
                shrink == Shrink::SectAbsorbAggr
            }
        }
    }

    /// `H5MF__add_sect` / `H5FS_sect_add` with `H5FS_ADD_RETURNED_SPACE`: merges with
    /// neighbours, shrinks the container, then links what is left (`H5FS__sect_merge`).
    fn add_sect(&mut self, t: Mem, addr: u64, size: u64) {
        let fi = Self::fs_index(t);
        let mut sect = Some((addr, size));
        // Merge with neighbours until nothing changes.
        loop {
            let (a, s) = sect.unwrap();
            let mut modified = false;
            let fs = self.fs[fi].as_mut().unwrap();
            let below = fs.sections.range(..a).next_back().map(|(&k, &v)| (k, v));
            let mut cur = (a, s);
            if let Some((ba, bs)) = below
                && ba + bs == a
            {
                fs.sections.remove(&ba);
                cur = (ba, bs + s);
                modified = true;
            }
            let above = fs.sections.range(cur.0 + 1..).next().map(|(&k, &v)| (k, v));
            if let Some((aa, asz)) = above
                && cur.0 + cur.1 == aa
            {
                fs.sections.remove(&aa);
                cur = (cur.0, cur.1 + asz);
                modified = true;
            }
            sect = Some(cur);
            if !modified {
                break;
            }
        }
        // Shrink until nothing changes.
        let mut remove_sect = false;
        while let Some(cur) = sect {
            let Some((shrink, raw)) = self.can_shrink(t, cur, false) else {
                break;
            };
            if remove_sect {
                self.fs[fi].as_mut().unwrap().sections.remove(&cur.0);
                remove_sect = false;
            }
            let mut c = cur;
            let survives = self.shrink(shrink, raw, &mut c, true);
            if survives {
                sect = Some(c);
            } else {
                // Shrunk away: try the last section of the merge list next.
                sect = self.fs[fi]
                    .as_ref()
                    .unwrap()
                    .sections
                    .iter()
                    .next_back()
                    .map(|(&k, &v)| (k, v));
                if sect.is_some() {
                    remove_sect = true;
                }
            }
        }
        if remove_sect {
            sect = None;
        }
        if let Some((a, s)) = sect {
            self.fs[fi].as_mut().unwrap().sections.insert(a, s);
        }
    }

    /// `H5MF_try_extend`: grows the block `addr`..`addr + size` by `extra` bytes, at the EOA,
    /// into the aggregator that follows it, or into a free section that follows it.
    pub(crate) fn try_extend(&mut self, t: Mem, addr: u64, size: u64, extra: u64) -> bool {
        let end = addr + size;
        if self.vfd_try_extend(end, extra) {
            return true;
        }
        let raw = t.raw();
        let eoa = self.eoa;
        let ag = *self.aggr(raw);
        if end == ag.addr {
            if eoa == ag.addr + ag.size {
                if extra <= (EXTEND_THRESHOLD * ag.size as f32) as u64 {
                    let a = self.aggr(raw);
                    a.size -= extra;
                    a.addr += extra;
                    return true;
                }
                let extra2 = if extra < ag.alloc_size {
                    ag.alloc_size
                } else {
                    extra
                };
                if self.vfd_try_extend(ag.addr + ag.size, extra2) {
                    let a = self.aggr(raw);
                    a.addr += extra;
                    a.tot_size += extra2;
                    a.size += extra2;
                    a.size -= extra;
                    return true;
                }
            } else if ag.size >= extra {
                let a = self.aggr(raw);
                a.size -= extra;
                a.addr += extra;
                return true;
            }
        }
        let fi = Self::fs_index(t);
        if let Some(fs) = self.fs[fi].as_mut()
            && let Some((&sa, &ss)) = fs.sections.range(addr..).next()
            && ss >= extra
            && end == sa
        {
            fs.sections.remove(&sa);
            if ss > extra {
                fs.sections.insert(sa + extra, ss - extra);
            }
            return true;
        }
        false
    }

    /// `H5MF_close` for a file without persistent free space: release the aggregators (the
    /// later one first), then shrink the EOA as far as free sections and aggregators at its
    /// end allow (`H5MF__close_aggrfs`, `H5MF__close_shrink_eoa`).
    pub(crate) fn close(&mut self) {
        self.free_aggrs();
        self.close_shrink_eoa();
        self.free_aggrs();
        self.close_shrink_eoa();
    }

    /// `H5MF_free_aggrs`.
    pub(crate) fn free_aggrs(&mut self) {
        let (ma, sa) = (self.meta.addr, self.sdata.addr);
        let first_raw = if ma != UNDEF && sa != UNDEF {
            ma < sa
        } else {
            false
        };
        self.aggr_reset(first_raw);
        self.aggr_reset(!first_raw);
    }

    /// `H5MF__aggr_reset`.
    fn aggr_reset(&mut self, raw: bool) {
        let ag = self.aggr(raw);
        let (addr, size) = (ag.addr, ag.size);
        ag.tot_size = 0;
        ag.addr = 0;
        ag.size = 0;
        if size > 0 {
            self.xfree(if raw { Mem::Draw } else { Mem::Default }, addr, size);
        }
    }

    fn close_shrink_eoa(&mut self) {
        loop {
            let mut shrank = false;
            for fi in 0..2 {
                let t = if fi == 1 { Mem::Draw } else { Mem::Default };
                let last = self.fs[fi]
                    .as_ref()
                    .and_then(|fs| fs.sections.iter().next_back().map(|(&k, &v)| (k, v)));
                if let Some(s) = last
                    && self.can_shrink(t, s, true).is_some()
                {
                    self.fs[fi].as_mut().unwrap().sections.remove(&s.0);
                    self.vfd_free(s.0, s.1);
                    shrank = true;
                }
            }
            for raw in [false, true] {
                let ag = *self.aggr(raw);
                if ag.size > 0 && ag.addr != UNDEF && self.eoa == ag.addr + ag.size {
                    self.vfd_free(ag.addr, ag.size);
                    let a = self.aggr(raw);
                    a.tot_size = 0;
                    a.addr = UNDEF;
                    a.size = 0;
                    shrank = true;
                }
            }
            if !shrank {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn itk_file_start() {
        // The superblock, the root group and a dataset header come from the metadata
        // aggregator; raw data starts a small-data block after it; a global heap of 4096
        // bytes extends the file at the small-data aggregator's end.
        let mut s = Space::new();
        assert_eq!(s.alloc(Mem::Super, 48), 0);
        assert_eq!(s.alloc(Mem::OHdr, 147), 48);
        assert_eq!(s.alloc(Mem::OHdr, 284), 195);
        assert_eq!(s.alloc(Mem::Draw, 16), 2048);
        assert_eq!(s.alloc(Mem::GHeap, 4096), 2064);
        assert_eq!(s.alloc(Mem::Draw, 16), 6160);
        assert_eq!(s.eoa, 8192);
    }
}
