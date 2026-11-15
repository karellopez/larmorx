// SPDX-License-Identifier: Apache-2.0 AND libtiff
//! `tif_dirread.c`: `TIFFReadDirectory` with its fetch functions, `EstimateStripByteCounts`
//! and `ChopUpSingleUncompressedStrip`, for files read without memory mapping.
//!
//! Sizes are computed as libtiff 3.6.1 computes them, in 32 bits (`tsize_t`, `uint32`). Where
//! a true size does not fit and libtiff would allocate the truncated size and then write past
//! it, the port refuses the allocation as `malloc` failing (FS-815).

use super::dir::{Arg, Data};
use super::fields::{self, FieldInfo, FieldRef, TIFF_SPP, TIFF_VARIABLE, bit, tag};
use super::io::Whence;
use super::{
    TIFF_BEENWRITING, TIFF_BIGENDIAN, TIFF_ISTILED, TIFF_STRIPCHOP, Tiff, Unported, compression,
    cut, values,
};
use larmorx_core::cfmt::Streams;
use larmorx_core::defects::{FS_815, FS_820, FS_821, FS_822, FS_823, FS_824, FS_825};

const MODULE: &str = "TIFFReadDirectory";

/// `TIFFDirEntry` after the byte swap of its first four bytes as shorts and its last eight as
/// longs; `raw` keeps the value field's bytes as the file holds them.
#[derive(Clone, Copy, Debug)]
pub(crate) struct DirEntry {
    pub tag: u16,
    pub ty: u16,
    pub count: u32,
    pub offset: u32,
    pub raw: [u8; 4],
}

/// `typemask` and the big-endian `typeshift` of `tif_open.c` (types past `TIFF_DOUBLE` read
/// past the tables: 0 here).
fn type_mask(ty: u16) -> u32 {
    match ty {
        1 | 6 | 7 => 0xff,
        3 | 8 => 0xffff,
        2 | 4 | 5 | 9 | 10 | 11 | 12 => 0xffff_ffff,
        _ => 0,
    }
}
fn type_shift_big(ty: u16) -> u32 {
    match ty {
        1 | 6 | 7 => 24,
        3 | 8 => 16,
        _ => 0,
    }
}

impl Tiff {
    /// `TIFFExtractData(tif, type, v)`.
    pub(crate) fn extract_data(&self, ty: u16, v: u32) -> u32 {
        if self.header.magic == TIFF_BIGENDIAN {
            (v >> type_shift_big(ty)) & type_mask(ty)
        } else {
            v & type_mask(ty)
        }
    }

    /// `CheckMalloc(tif, n, what)` with `n` as the caller computes it in `size_t`, passed as a
    /// `tsize_t`: whether the allocation succeeds (its message written when not). A size that
    /// does not fit 31 bits but whose truncation does is refused too (FS-815).
    pub(crate) fn check_malloc(&mut self, n: u64, what: &str, s: &mut dyn Streams) -> bool {
        let t = n as u32 as i32;
        let ok = if t < 0 {
            false
        } else if u64::from(t as u32) != n {
            self.notices.add(
                &FS_815,
                format!(
                    "{}: a size of {n} bytes {what} does not fit libtiff's 32-bit sizes: libtiff \
                     allocates {t} bytes and writes past them (a crash); refused as an \
                     allocation failure",
                    self.name
                ),
            );
            false
        } else {
            self.can_alloc(n)
        };
        if !ok {
            let name = self.name.clone();
            self.error(Some(&name), &format!("No space {what}"), s);
        }
        ok
    }

    /// `TIFFReadDirectory(tif)`.
    pub fn read_directory(&mut self, s: &mut dyn Streams) -> Result<bool, Unported> {
        let r = self.read_directory_inner(s);
        match self.unported {
            Some(u) => Err(u),
            None => Ok(r),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn read_directory_inner(&mut self, s: &mut dyn Streams) -> bool {
        self.diroff = self.nextdiroff;
        if self.diroff == 0 {
            return false;
        }
        if self.dirlist[..usize::from(self.dirnumber).min(self.dirlist.len())]
            .contains(&self.diroff)
        {
            return false;
        }
        self.dirnumber = self.dirnumber.wrapping_add(1);
        self.dirlist.truncate(usize::from(self.dirnumber) - 1);
        self.dirlist.push(self.diroff);
        self.cleanup();
        self.curdir = self.curdir.wrapping_add(1);
        let name = self.name.clone();
        if !self.seek_ok(self.diroff) {
            let m = format!("{}: Seek error accessing TIFF directory", cut(&name, 1000));
            self.error(Some(MODULE), &m, s);
            return false;
        }
        let mut c = [0u8; 2];
        if !self.read_ok(&mut c, 2) {
            let m = format!("{}: Can not read TIFF directory count", cut(&name, 1000));
            self.error(Some(MODULE), &m, s);
            return false;
        }
        let mut dircount = u16::from_le_bytes(c);
        if self.swab() {
            dircount = dircount.swap_bytes();
        }
        let nbytes = u64::from(dircount) * 12;
        if !self.check_malloc(nbytes, "to read TIFF directory", s) {
            return false;
        }
        let mut raw = vec![0u8; nbytes as usize];
        if !self.read_ok(&mut raw, nbytes as i32) {
            let m = format!("{}: Can not read TIFF directory", cut(&name, 100));
            self.error(Some(MODULE), &m, s);
            return false;
        }
        let mut l = [0u8; 4];
        self.fd.read(&mut l, 4);
        let mut nextdiroff = u32::from_le_bytes(l);
        if self.swab() {
            nextdiroff = nextdiroff.swap_bytes();
        }
        self.nextdiroff = nextdiroff;
        self.flags &= !TIFF_BEENWRITING;
        self.free_directory();
        if self.default_directory(s).is_err() {
            return false;
        }
        self.set_field_arg(tag::PLANARCONFIG, Arg::Int(1), s);
        let swab = self.swab();
        let mut dir: Vec<DirEntry> = raw
            .chunks_exact(12)
            .map(|e| {
                let mut tg = u16::from_le_bytes([e[0], e[1]]);
                let mut ty = u16::from_le_bytes([e[2], e[3]]);
                let mut count = u32::from_le_bytes([e[4], e[5], e[6], e[7]]);
                let mut offset = u32::from_le_bytes([e[8], e[9], e[10], e[11]]);
                if swab {
                    tg = tg.swap_bytes();
                    ty = ty.swap_bytes();
                    count = count.swap_bytes();
                    offset = offset.swap_bytes();
                }
                DirEntry {
                    tag: tg,
                    ty,
                    count,
                    offset,
                    raw: [e[8], e[9], e[10], e[11]],
                }
            })
            .collect();
        // The pass for SamplesPerPixel.
        for i in 0..dir.len() {
            if u32::from(dir[i].tag) == tag::SAMPLESPERPIXEL {
                let d = dir[i];
                if !self.fetch_normal_tag(&d, s) {
                    return false;
                }
                dir[i].tag = 0;
            }
        }
        if self.unported.is_some() {
            return false;
        }
        // The first real pass.
        let mut fix: usize = 0;
        let mut out_of_order_warned = false;
        let mut unchecked: Vec<usize> = Vec::new();
        for i in 0..dir.len() {
            let dp = dir[i];
            let nfields = self.fields.sorted.len();
            if dp.tag == 0 {
                continue;
            }
            if fix >= nfields {
                if self.fix() {
                    fix = 0;
                } else {
                    unchecked.push(i);
                    continue;
                }
            }
            let t = u32::from(dp.tag);
            if t < self.fields.info(self.fields.sorted[fix]).tag {
                if !out_of_order_warned {
                    let m = format!(
                        "{}: invalid TIFF directory; tags are not sorted in ascending order",
                        cut(&name, 1000)
                    );
                    self.warning(Some(MODULE), &m, s);
                    out_of_order_warned = true;
                }
                fix = 0;
            }
            while fix < nfields && self.fields.info(self.fields.sorted[fix]).tag < t {
                fix += 1;
            }
            if fix >= nfields || self.fields.info(self.fields.sorted[fix]).tag != t {
                let m = format!(
                    "{}: unknown field with tag {} ({:#x}) encountered",
                    cut(&name, 1000),
                    dp.tag,
                    dp.tag
                );
                self.warning(Some(MODULE), &m, s);
                self.fields.merge_anon(t, dp.ty);
                fix = 0;
                let nfields = self.fields.sorted.len();
                while fix < nfields && self.fields.info(self.fields.sorted[fix]).tag < t {
                    fix += 1;
                }
            }
            let nfields = self.fields.sorted.len();
            let mut fref = self.fields.sorted[fix];
            if self.fields.info(fref).bit == bit::IGNORE {
                dir[i].tag = 0;
                continue;
            }
            // The type check walks the table in memory (`fip++`).
            let mut ignore = false;
            while dp.ty != self.fields.info(fref).ty {
                if self.fields.info(fref).ty == fields::TIFF_ANY {
                    break;
                }
                let prev_name = self.fields.info(fref).name.clone();
                let next = fields::Fields::next_in_memory(fref);
                fix += 1;
                let mismatch = match next {
                    None => {
                        if fix != nfields {
                            self.notices.add(
                                &FS_820,
                                format!(
                                    "{}: the type check of tag {} read past the end of a field \
                                     table (memory contents), taken as no further entry",
                                    self.name, dp.tag
                                ),
                            );
                        }
                        true
                    }
                    Some(n) => {
                        fref = n;
                        fix == nfields || self.fields.info(n).tag != t
                    }
                };
                if mismatch {
                    let m = format!(
                        "{}: wrong data type {} for \"{}\"; tag ignored",
                        cut(&name, 1000),
                        dp.ty,
                        prev_name
                    );
                    self.warning(Some(MODULE), &m, s);
                    ignore = true;
                    break;
                }
            }
            if ignore {
                dir[i].tag = 0;
                continue;
            }
            let fip: FieldInfo = self.fields.info(fref).clone();
            if fip.readcount != TIFF_VARIABLE {
                let expected = if fip.readcount == TIFF_SPP {
                    u32::from(self.dir.samplesperpixel)
                } else {
                    fip.readcount as i32 as u32
                };
                if !self.check_dir_count(&dp, expected, s) {
                    dir[i].tag = 0;
                    continue;
                }
            }
            match t {
                tag::COMPRESSION => {
                    if dp.count == 1 {
                        let v = self.extract_data(dp.ty, dp.offset);
                        if !self.set_field_arg(t, Arg::Int(v), s) {
                            return false;
                        }
                        if self.unported.is_some() {
                            return false;
                        }
                        continue;
                    }
                    let Some(iv) = self.fetch_per_sample_shorts(&dp, s) else {
                        return false;
                    };
                    if !self.set_field_arg(t, Arg::Int(iv as u32), s) {
                        return false;
                    }
                    if self.unported.is_some() {
                        return false;
                    }
                    dir[i].tag = 0;
                }
                tag::STRIPOFFSETS | tag::STRIPBYTECOUNTS | tag::TILEOFFSETS
                | tag::TILEBYTECOUNTS => self.dir.set_bit(fip.bit),
                tag::IMAGEWIDTH
                | tag::IMAGELENGTH
                | tag::IMAGEDEPTH
                | tag::TILELENGTH
                | tag::TILEWIDTH
                | tag::TILEDEPTH
                | tag::PLANARCONFIG
                | tag::ROWSPERSTRIP => {
                    if !self.fetch_normal_tag(&dp, s) {
                        return false;
                    }
                    dir[i].tag = 0;
                }
                tag::EXTRASAMPLES => {
                    self.fetch_extra_samples(&dp, s);
                    dir[i].tag = 0;
                }
                _ => {}
            }
        }
        if !self.dir.is_set(bit::IMAGEDIMENSIONS) {
            self.missing_required("ImageLength", s);
            return false;
        }
        if !self.dir.is_set(bit::PLANARCONFIG) {
            self.missing_required("PlanarConfiguration", s);
            return false;
        }
        if !self.dir.is_set(bit::TILEDIMENSIONS) {
            self.dir.nstrips = self.number_of_strips();
            self.dir.tilewidth = self.dir.imagewidth;
            self.dir.tilelength = self.dir.rowsperstrip;
            self.dir.tiledepth = self.dir.imagedepth;
            self.flags &= !TIFF_ISTILED;
        } else {
            self.dir.nstrips = self.number_of_tiles();
            self.flags |= TIFF_ISTILED;
        }
        self.dir.stripsperimage = self.dir.nstrips;
        if self.dir.planarconfig == values::PLANARCONFIG_SEPARATE {
            self.dir.stripsperimage /= u32::from(self.dir.samplesperpixel);
        }
        if !self.dir.is_set(bit::STRIPOFFSETS) {
            let what = if self.is_tiled() {
                "TileOffsets"
            } else {
                "StripOffsets"
            };
            self.missing_required(what, s);
            return false;
        }
        // The second pass.
        for i in 0..dir.len() {
            let dp = dir[i];
            if dp.tag == 0 {
                continue;
            }
            let t = u32::from(dp.tag);
            if unchecked.contains(&i) && self.fields.find(t).is_none() {
                // libtiff 3.6.1 skipped the type check of this entry (a previous entry's
                // check ran to the end of its table) and now fetches an unregistered tag: it
                // dereferences a null pointer (a crash).
                self.notices.add(
                    &FS_820,
                    format!(
                        "{}: tag {} was never registered (its directory's type check had \
                         stopped): libtiff 3.6.1 crashes on it; ignored",
                        self.name, dp.tag
                    ),
                );
                continue;
            }
            if unchecked.contains(&i) {
                self.notices.add(
                    &FS_820,
                    format!(
                        "{}: tag {} is read without its type and count checked (libtiff 3.6.1 \
                         stops checking after a type check that ran to the end of a table)",
                        self.name, dp.tag
                    ),
                );
            }
            match t {
                tag::MINSAMPLEVALUE | tag::MAXSAMPLEVALUE | tag::BITSPERSAMPLE
                    if dp.count == 1 =>
                {
                    let v = self.extract_data(dp.ty, dp.offset);
                    if !self.set_field_arg(t, Arg::Int(v), s) {
                        return false;
                    }
                }
                tag::MINSAMPLEVALUE
                | tag::MAXSAMPLEVALUE
                | tag::BITSPERSAMPLE
                | tag::DATATYPE
                | tag::SAMPLEFORMAT => {
                    let Some(iv) = self.fetch_per_sample_shorts(&dp, s) else {
                        return false;
                    };
                    if !self.set_field_arg(t, Arg::Int(iv as u32), s) {
                        return false;
                    }
                }
                tag::SMINSAMPLEVALUE | tag::SMAXSAMPLEVALUE => {
                    let Some(dv) = self.fetch_per_sample_anys(&dp, s) else {
                        return false;
                    };
                    if !self.set_field_arg(t, Arg::Dbl(dv), s) {
                        return false;
                    }
                }
                tag::STRIPOFFSETS | tag::TILEOFFSETS => {
                    if !self.fetch_strip_thing(&dp, false, s) {
                        return false;
                    }
                }
                tag::STRIPBYTECOUNTS | tag::TILEBYTECOUNTS => {
                    if !self.fetch_strip_thing(&dp, true, s) {
                        return false;
                    }
                }
                tag::COLORMAP | tag::TRANSFERFUNCTION => self.fetch_colormap(&dp, s),
                tag::PAGENUMBER | tag::HALFTONEHINTS | tag::YCBCRSUBSAMPLING | tag::DOTRANGE => {
                    self.fetch_short_pair(&dp, s);
                }
                tag::REFERENCEBLACKWHITE => {
                    self.fetch_ref_black_white(&dp, s);
                }
                tag::OSUBFILETYPE => {
                    let v = match self.extract_data(dp.ty, dp.offset) {
                        values::OFILETYPE_REDUCEDIMAGE => values::FILETYPE_REDUCEDIMAGE,
                        values::OFILETYPE_PAGE => values::FILETYPE_PAGE,
                        _ => 0,
                    };
                    if v != 0 {
                        self.set_field_arg(tag::SUBFILETYPE, Arg::Int(v), s);
                    }
                }
                _ => {
                    self.fetch_normal_tag(&dp, s);
                }
            }
        }
        if self.dir.photometric == values::PHOTOMETRIC_PALETTE && !self.dir.is_set(bit::COLORMAP)
        {
            self.missing_required("Colormap", s);
            return false;
        }
        if !self.dir.is_set(bit::STRIPBYTECOUNTS) {
            let d = &self.dir;
            if (d.planarconfig == values::PLANARCONFIG_CONTIG && d.nstrips > 1)
                || (d.planarconfig == values::PLANARCONFIG_SEPARATE
                    && d.nstrips != u32::from(d.samplesperpixel))
            {
                self.missing_required("StripByteCounts", s);
                return false;
            }
            let f = self.field_name_of(tag::STRIPBYTECOUNTS, s);
            let m = format!(
                "{}: TIFF directory is missing required \"{f}\" field, calculating from \
                 imagelength",
                cut(&name, 1000)
            );
            self.warning(Some(MODULE), &m, s);
            if !self.estimate_strip_byte_counts(&dir, s) {
                return false;
            }
        } else if self.dir.nstrips == 1 && self.bytecount_looks_bad() {
            let f = self.field_name_of(tag::STRIPBYTECOUNTS, s);
            let m = format!(
                "{}: Bogus \"{f}\" field, ignoring and calculating from imagelength",
                cut(&name, 1000)
            );
            self.warning(Some(MODULE), &m, s);
            if !self.estimate_strip_byte_counts(&dir, s) {
                return false;
            }
        }
        if !self.dir.is_set(bit::MAXSAMPLEVALUE) {
            self.dir.maxsamplevalue =
                ((1i64 << (self.dir.bitspersample & 63)).wrapping_sub(1)) as u16;
        }
        if !self.dir.is_set(bit::COMPRESSION) {
            self.set_field_arg(tag::COMPRESSION, Arg::Int(u32::from(compression::NONE)), s);
        }
        if self.dir.nstrips == 1
            && self.dir.compression == compression::NONE
            && self.flags & (TIFF_STRIPCHOP | TIFF_ISTILED) == TIFF_STRIPCHOP
        {
            self.chop_up_single_uncompressed_strip(s);
        }
        self.row = u32::MAX;
        self.curstrip = u32::MAX;
        self.col = u32::MAX;
        self.curtile = u32::MAX;
        self.tilesize = self.tile_size();
        self.scanlinesize = self.scanline_size();
        true
    }

    fn field_name_of(&self, t: u32, s: &mut dyn Streams) -> String {
        self.field_with_tag(t, s)
            .map(|f| f.name.into_owned())
            .unwrap_or_default()
    }

    /// `BYTECOUNTLOOKSBAD`.
    fn bytecount_looks_bad(&self) -> bool {
        let d = &self.dir;
        let c0 = d.stripbytecount.as_ref().and_then(|v| v.first()).copied().unwrap_or(0);
        let o0 = d.stripoffset.as_ref().and_then(|v| v.first()).copied().unwrap_or(0);
        (c0 == 0 && o0 != 0)
            || (d.compression == compression::NONE && c0 > self.fd.size().wrapping_sub(o0))
    }

    /// `MissingRequired(tif, tagname)`.
    fn missing_required(&self, tagname: &str, s: &mut dyn Streams) {
        let m = format!(
            "{}: TIFF directory is missing required \"{tagname}\" field",
            cut(&self.name, 1000)
        );
        self.error(Some("MissingRequired"), &m, s);
    }

    /// `CheckDirCount(tif, dir, count)`.
    fn check_dir_count(&self, dp: &DirEntry, count: u32, s: &mut dyn Streams) -> bool {
        if count != dp.count {
            let f = self
                .field_with_tag(u32::from(dp.tag), s)
                .map(|f| f.name.into_owned())
                .unwrap_or_default();
            let m = format!(
                "incorrect count for field \"{f}\" ({}, expecting {count}); tag ignored",
                dp.count
            );
            self.warning(Some(&self.name.clone()), &m, s);
            return false;
        }
        true
    }

    /// `TIFFFetchData(tif, dir, cp)`: the bytes read, swapped to the machine's order (`None`
    /// with its message on failure). `cap`: the bytes the caller's buffer holds.
    fn fetch_data(&mut self, dp: &DirEntry, s: &mut dyn Streams) -> Option<Vec<u8>> {
        let w = fields::data_width(dp.ty);
        let cc = dp.count.wrapping_mul(w as u32) as i32;
        let ok = if !self.seek_ok(dp.offset) {
            None
        } else {
            // Read at most what the file still holds (plus one byte: a short read fails).
            let left = u64::from(self.fd.size().saturating_sub(dp.offset)) + 1;
            let want = if cc < 0 { 0 } else { (cc as u64).min(left) as usize };
            let mut buf = vec![0u8; want];
            let got = self.fd.read(&mut buf, cc);
            if got == cc {
                buf.truncate(cc.max(0) as usize);
                Some(buf)
            } else {
                None
            }
        };
        let Some(mut buf) = ok else {
            let f = self
                .field_with_tag(u32::from(dp.tag), s)
                .map(|f| f.name.into_owned())
                .unwrap_or_default();
            let m = format!("Error fetching data for field \"{f}\"");
            self.error(Some(&self.name.clone()), &m, s);
            return None;
        };
        if self.swab() {
            match dp.ty {
                fields::TIFF_SHORT | fields::TIFF_SSHORT => {
                    buf.chunks_exact_mut(2).for_each(|c| c.swap(0, 1));
                }
                fields::TIFF_LONG
                | fields::TIFF_SLONG
                | fields::TIFF_FLOAT
                | fields::TIFF_RATIONAL
                | fields::TIFF_SRATIONAL => buf.chunks_exact_mut(4).for_each(|c| c.reverse()),
                fields::TIFF_DOUBLE => buf.chunks_exact_mut(8).for_each(|c| c.reverse()),
                _ => {}
            }
        }
        Some(buf)
    }

    /// `TIFFFetchString(tif, dir, cp)`: the bytes of the string (without the NUL added later).
    fn fetch_string(&mut self, dp: &DirEntry, s: &mut dyn Streams) -> Option<Vec<u8>> {
        if dp.count <= 4 {
            return Some(dp.raw[..dp.count as usize].to_vec());
        }
        self.fetch_data(dp, s)
    }

    /// `cvtRational`.
    fn cvt_rational(&self, dp: &DirEntry, num: u32, denom: u32, s: &mut dyn Streams) -> Option<f32> {
        if denom == 0 {
            let f = self
                .field_with_tag(u32::from(dp.tag), s)
                .map(|f| f.name.into_owned())
                .unwrap_or_default();
            let m = format!("{f}: Rational with zero denominator (num = {num})");
            self.error(Some(&self.name.clone()), &m, s);
            None
        } else if dp.ty == fields::TIFF_RATIONAL {
            Some(num as f32 / denom as f32)
        } else {
            Some(num as i32 as f32 / denom as i32 as f32)
        }
    }

    /// `TIFFFetchRational`: 1.0 on failure.
    fn fetch_rational(&mut self, dp: &DirEntry, s: &mut dyn Streams) -> f32 {
        let Some(b) = self.fetch_data(dp, s) else {
            return 1.0;
        };
        let l0 = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        let l1 = u32::from_le_bytes([b[4], b[5], b[6], b[7]]);
        self.cvt_rational(dp, l0, l1, s).unwrap_or(1.0)
    }

    /// `TIFFFetchByteArray`: `count` values in a `uint16` array. Past four values libtiff reads
    /// the bytes into the first half of the array, so the values are byte pairs and the rest is
    /// memory never written (FS-824: here 0; corrected, the bytes themselves).
    fn fetch_byte_array(&mut self, dp: &DirEntry, s: &mut dyn Streams) -> Option<Vec<u16>> {
        let n = dp.count as usize;
        if dp.count <= 4 {
            let o = dp.offset;
            let big = self.header.magic == TIFF_BIGENDIAN;
            let sbyte = dp.ty == fields::TIFF_SBYTE;
            let mut v = vec![0u16; 4];
            for (i, x) in v.iter_mut().enumerate().take(n) {
                let b = if big {
                    (o >> (24 - 8 * i)) & 0xff
                } else {
                    (o >> (8 * i)) & 0xff
                } as u8;
                *x = if sbyte { b as i8 as i16 as u16 } else { u16::from(b) };
            }
            v.truncate(n);
            return Some(v);
        }
        let b = self.fetch_data(dp, s)?;
        if self.fix() {
            let sbyte = dp.ty == fields::TIFF_SBYTE;
            return Some(
                b.iter()
                    .map(|&x| if sbyte { x as i8 as i16 as u16 } else { u16::from(x) })
                    .collect(),
            );
        }
        let mut v = vec![0u16; n];
        for (i, x) in v.iter_mut().enumerate().take(n / 2) {
            *x = u16::from_le_bytes([b[2 * i], b[2 * i + 1]]);
        }
        if n % 2 == 1 {
            v[n / 2] = u16::from(b[n - 1]);
        }
        Some(v)
    }

    fn byte_array_notice(&mut self, dp: &DirEntry) {
        if dp.count > 4 {
            let m = format!(
                "{}: tag {} holds {} BYTE values: libtiff 3.6.1 reads them as {} 16-bit values \
                 and the rest of its array is memory never written",
                self.name,
                dp.tag,
                dp.count,
                dp.count / 2
            );
            self.notices.add(&FS_824, m);
        }
    }

    /// `TIFFFetchShortArray`.
    fn fetch_short_array(&mut self, dp: &DirEntry, s: &mut dyn Streams) -> Option<Vec<u16>> {
        if dp.count <= 2 {
            let o = dp.offset;
            let (a, b) = if self.header.magic == TIFF_BIGENDIAN {
                ((o >> 16) as u16, (o & 0xffff) as u16)
            } else {
                ((o & 0xffff) as u16, (o >> 16) as u16)
            };
            return Some([a, b][..dp.count as usize].to_vec());
        }
        let b = self.fetch_data(dp, s)?;
        let mut v: Vec<u16> = b
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        v.resize(dp.count as usize, 0);
        Some(v)
    }

    /// `TIFFFetchLongArray`.
    fn fetch_long_array(&mut self, dp: &DirEntry, s: &mut dyn Streams) -> Option<Vec<u32>> {
        if dp.count == 1 {
            return Some(vec![dp.offset]);
        }
        let b = self.fetch_data(dp, s)?;
        Some(
            b.chunks_exact(4)
                .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect(),
        )
    }

    /// `TIFFFetchRationalArray`.
    fn fetch_rational_array(&mut self, dp: &DirEntry, s: &mut dyn Streams) -> Option<Vec<f32>> {
        let n = u64::from(dp.count) * fields::data_width(dp.ty) as u64;
        if !self.check_malloc(n, "to fetch array of rationals", s) {
            return None;
        }
        let b = self.fetch_data(dp, s)?;
        let mut v = Vec::with_capacity(dp.count as usize);
        for i in 0..dp.count as usize {
            let num = u32::from_le_bytes([b[8 * i], b[8 * i + 1], b[8 * i + 2], b[8 * i + 3]]);
            let den = u32::from_le_bytes([b[8 * i + 4], b[8 * i + 5], b[8 * i + 6], b[8 * i + 7]]);
            v.push(self.cvt_rational(dp, num, den, s)?);
        }
        Some(v)
    }

    /// `TIFFFetchFloatArray`.
    fn fetch_float_array(&mut self, dp: &DirEntry, s: &mut dyn Streams) -> Option<Vec<f32>> {
        if dp.count == 1 {
            return Some(vec![f32::from_bits(dp.offset)]);
        }
        let b = self.fetch_data(dp, s)?;
        Some(
            b.chunks_exact(4)
                .map(|c| f32::from_bits(u32::from_le_bytes([c[0], c[1], c[2], c[3]])))
                .collect(),
        )
    }

    /// `TIFFFetchDoubleArray`.
    fn fetch_double_array(&mut self, dp: &DirEntry, s: &mut dyn Streams) -> Option<Vec<f64>> {
        let b = self.fetch_data(dp, s)?;
        Some(
            b.chunks_exact(8)
                .map(|c| f64::from_bits(u64::from_le_bytes(c.try_into().expect("8 bytes"))))
                .collect(),
        )
    }

    /// `TIFFFetchAnyArray`: the values as `double`s.
    fn fetch_any_array(&mut self, dp: &DirEntry, s: &mut dyn Streams) -> Option<Vec<f64>> {
        Some(match dp.ty {
            fields::TIFF_BYTE | fields::TIFF_SBYTE => {
                let v = self.fetch_byte_array(dp, s)?;
                if dp.ty == fields::TIFF_BYTE {
                    v.iter().map(|&x| f64::from(x)).collect()
                } else {
                    v.iter().map(|&x| f64::from(x as i16)).collect()
                }
            }
            fields::TIFF_SHORT | fields::TIFF_SSHORT => {
                let v = self.fetch_short_array(dp, s)?;
                if dp.ty == fields::TIFF_SHORT {
                    v.iter().map(|&x| f64::from(x)).collect()
                } else {
                    v.iter().map(|&x| f64::from(x as i16)).collect()
                }
            }
            fields::TIFF_LONG | fields::TIFF_SLONG => {
                let v = self.fetch_long_array(dp, s)?;
                if dp.ty == fields::TIFF_LONG {
                    v.iter().map(|&x| f64::from(x)).collect()
                } else {
                    v.iter().map(|&x| f64::from(x as i32)).collect()
                }
            }
            fields::TIFF_RATIONAL | fields::TIFF_SRATIONAL => self
                .fetch_rational_array(dp, s)?
                .iter()
                .map(|&x| f64::from(x))
                .collect(),
            fields::TIFF_FLOAT => self
                .fetch_float_array(dp, s)?
                .iter()
                .map(|&x| f64::from(x))
                .collect(),
            fields::TIFF_DOUBLE => self.fetch_double_array(dp, s)?,
            _ => {
                let f = self
                    .field_with_tag(u32::from(dp.tag), s)
                    .map(|f| f.name.into_owned())
                    .unwrap_or_default();
                self.notices.add(
                    &FS_823,
                    format!(
                        "{}: libtiff 3.6.1's message for a field of type {} passes no type: \
                         its %d and %s read memory never given (a crash or garbage); written \
                         with the type",
                        self.name, dp.ty
                    ),
                );
                let m = format!("cannot read TIFF_ANY type {} for field \"{f}\"", dp.ty);
                self.error(Some(&self.name.clone()), &m, s);
                return None;
            }
        })
    }

    /// `TIFFFetchNormalTag(tif, dp)`.
    fn fetch_normal_tag(&mut self, dp: &DirEntry, s: &mut dyn Streams) -> bool {
        const MESG: &str = "to fetch tag value";
        let t = u32::from(dp.tag);
        let Some(fip) = self.field_with_tag(t, s) else {
            return false;
        };
        if dp.count > 1 {
            let n = u64::from(dp.count);
            let ok: Option<bool> = match dp.ty {
                fields::TIFF_BYTE | fields::TIFF_SBYTE => {
                    if !self.check_malloc(n * 2, MESG, s) {
                        return false;
                    }
                    self.fetch_byte_array(dp, s)
                        .map(|v| self.set_with(&fip, dp, Data::U16(&v), s))
                }
                fields::TIFF_SHORT | fields::TIFF_SSHORT => {
                    if !self.check_malloc(n * 2, MESG, s) {
                        return false;
                    }
                    self.fetch_short_array(dp, s)
                        .map(|v| self.set_with(&fip, dp, Data::U16(&v), s))
                }
                fields::TIFF_LONG | fields::TIFF_SLONG => {
                    if !self.check_malloc(n * 4, MESG, s) {
                        return false;
                    }
                    self.fetch_long_array(dp, s)
                        .map(|v| self.set_with(&fip, dp, Data::U32(&v), s))
                }
                fields::TIFF_RATIONAL | fields::TIFF_SRATIONAL => {
                    if !self.check_malloc(n * 4, MESG, s) {
                        return false;
                    }
                    self.fetch_rational_array(dp, s)
                        .map(|v| self.set_with(&fip, dp, Data::F32(&v), s))
                }
                fields::TIFF_FLOAT => {
                    if !self.check_malloc(n * 4, MESG, s) {
                        return false;
                    }
                    self.fetch_float_array(dp, s)
                        .map(|v| self.set_with(&fip, dp, Data::F32(&v), s))
                }
                fields::TIFF_DOUBLE => {
                    if !self.check_malloc(n * 8, MESG, s) {
                        return false;
                    }
                    self.fetch_double_array(dp, s)
                        .map(|v| self.set_with(&fip, dp, Data::F64(&v), s))
                }
                fields::TIFF_ASCII | fields::TIFF_UNDEFINED => {
                    if !self.check_malloc(u64::from(dp.count.wrapping_add(1)), MESG, s) {
                        return false;
                    }
                    if dp.count == u32::MAX {
                        self.notices.add(
                            &FS_815,
                            format!(
                                "{}: a string of 4294967295 bytes: libtiff 3.6.1 allocates 0 \
                                 and writes its NUL 4 GB past it (a crash); refused",
                                self.name
                            ),
                        );
                        return false;
                    }
                    self.fetch_string(dp, s).map(|mut v| {
                        v.truncate(dp.count as usize);
                        let a = if fip.passcount {
                            Arg::Count(dp.count, Data::Bytes(&v))
                        } else {
                            let z = v.iter().position(|&b| b == 0).unwrap_or(v.len());
                            Arg::Str(&v[..z])
                        };
                        self.set_field_arg(t, a, s)
                    })
                }
                _ => None,
            };
            ok.unwrap_or(false)
        } else if self.check_dir_count(dp, 1, s) {
            match dp.ty {
                fields::TIFF_BYTE
                | fields::TIFF_SBYTE
                | fields::TIFF_SHORT
                | fields::TIFF_SSHORT
                    if fip.ty != fields::TIFF_LONG && fip.ty != fields::TIFF_SLONG =>
                {
                    let v = self.extract_data(dp.ty, dp.offset) as u16;
                    if fip.passcount {
                        self.set_field_arg(t, Arg::Count(1, Data::U16(&[v])), s)
                    } else {
                        self.set_field_arg(t, Arg::Int(u32::from(v)), s)
                    }
                }
                fields::TIFF_BYTE
                | fields::TIFF_SBYTE
                | fields::TIFF_SHORT
                | fields::TIFF_SSHORT
                | fields::TIFF_LONG
                | fields::TIFF_SLONG => {
                    let v = self.extract_data(dp.ty, dp.offset);
                    if fip.passcount {
                        self.set_field_arg(t, Arg::Count(1, Data::U32(&[v])), s)
                    } else {
                        self.set_field_arg(t, Arg::Int(v), s)
                    }
                }
                fields::TIFF_RATIONAL | fields::TIFF_SRATIONAL | fields::TIFF_FLOAT => {
                    let v = if dp.ty == fields::TIFF_FLOAT {
                        f32::from_bits(self.extract_data(dp.ty, dp.offset))
                    } else {
                        self.fetch_rational(dp, s)
                    };
                    if fip.passcount {
                        self.set_field_arg(t, Arg::Count(1, Data::F32(&[v])), s)
                    } else {
                        self.set_field_arg(t, Arg::Dbl(f64::from(v)), s)
                    }
                }
                fields::TIFF_DOUBLE => match self.fetch_double_array(dp, s) {
                    Some(v) => {
                        let d = v.first().copied().unwrap_or(0.0);
                        if fip.passcount {
                            self.set_field_arg(t, Arg::Count(1, Data::F64(&[d])), s)
                        } else {
                            self.set_field_arg(t, Arg::Dbl(d), s)
                        }
                    }
                    None => false,
                },
                fields::TIFF_ASCII | fields::TIFF_UNDEFINED => {
                    match self.fetch_string(dp, s) {
                        Some(v) => {
                            let c = v.first().copied().unwrap_or(0);
                            let z: &[u8] = if c == 0 { &[] } else { &[c] };
                            let st = [c, 0];
                            let a = if fip.passcount {
                                // libtiff passes the string where the count goes (FS-825):
                                // the count and the value become stack contents.
                                self.notices.add(
                                    &FS_825,
                                    format!(
                                        "{}: tag {} is one character of a field that passes its \
                                         count: libtiff 3.6.1 passes a stack address as the \
                                         count (an allocation of any size, a copy from a \
                                         pointer never given); set as one value",
                                        self.name, dp.tag
                                    ),
                                );
                                Arg::Count(1, Data::Bytes(&st[..1]))
                            } else {
                                Arg::Str(z)
                            };
                            self.set_field_arg(t, a, s)
                        }
                        None => false,
                    }
                }
                _ => false,
            }
        } else {
            false
        }
    }

    /// `TIFFSetField(tif, tag, count, cp)` or `(tif, tag, cp)` after an array fetch.
    fn set_with(&mut self, fip: &FieldInfo, dp: &DirEntry, d: Data, s: &mut dyn Streams) -> bool {
        let a = if fip.passcount {
            Arg::Count(dp.count, d)
        } else {
            Arg::Ptr(d)
        };
        self.set_field_arg(u32::from(dp.tag), a, s)
    }

    /// `TIFFFetchPerSampleShorts`: the one value of every sample.
    fn fetch_per_sample_shorts(&mut self, dp: &DirEntry, s: &mut dyn Streams) -> Option<i32> {
        let samples = u32::from(self.dir.samplesperpixel);
        if !self.check_dir_count(dp, samples, s) {
            return None;
        }
        let v: Vec<u16> = if dp.ty == fields::TIFF_LONG || dp.ty == fields::TIFF_SLONG {
            // libtiff reads LONG values as shorts: the halves of the longs (FS-822).
            if self.fix() {
                self.notices.add(
                    &FS_822,
                    format!(
                        "{}: tag {} holds {samples} LONG values: read as LONG values (libtiff \
                         3.6.1 reads them as 16-bit halves)",
                        self.name, dp.tag
                    ),
                );
                self.fetch_long_array(dp, s)?
                    .iter()
                    .map(|&x| x as u16)
                    .collect()
            } else if dp.count <= 2 {
                self.fetch_short_array(dp, s)?
            } else {
                let b = self.fetch_data(dp, s)?;
                if samples > 5 {
                    self.notices.add(
                        &FS_822,
                        format!(
                            "{}: tag {} holds {samples} LONG values: libtiff 3.6.1 reads {} \
                             bytes into a buffer of {} (memory outside it)",
                            self.name,
                            dp.tag,
                            4 * samples,
                            2 * samples.max(10)
                        ),
                    );
                }
                let mut v: Vec<u16> = b
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect();
                v.truncate(samples as usize);
                v
            }
        } else {
            self.fetch_short_array(dp, s)?
        };
        let v0 = v.first().copied().unwrap_or(0);
        for &x in v.iter().skip(1).take(samples as usize - 1) {
            if x != v0 {
                let f = self
                    .field_with_tag(u32::from(dp.tag), s)
                    .map(|f| f.name.into_owned())
                    .unwrap_or_default();
                let m = format!("Cannot handle different per-sample values for field \"{f}\"");
                self.error(Some(&self.name.clone()), &m, s);
                return None;
            }
        }
        Some(i32::from(v0))
    }

    /// `TIFFFetchPerSampleAnys`.
    fn fetch_per_sample_anys(&mut self, dp: &DirEntry, s: &mut dyn Streams) -> Option<f64> {
        let samples = u32::from(self.dir.samplesperpixel);
        if !self.check_dir_count(dp, samples, s) {
            return None;
        }
        let v = self.fetch_any_array(dp, s)?;
        let v0 = v.first().copied().unwrap_or(0.0);
        for &x in v.iter().skip(1).take(samples as usize - 1) {
            if x != v0 {
                let f = self
                    .field_with_tag(u32::from(dp.tag), s)
                    .map(|f| f.name.into_owned())
                    .unwrap_or_default();
                let m = format!("Cannot handle different per-sample values for field \"{f}\"");
                self.error(Some(&self.name.clone()), &m, s);
                return None;
            }
        }
        Some(v0)
    }

    /// `TIFFFetchStripThing(tif, dir, nstrips, lpp)`.
    fn fetch_strip_thing(&mut self, dp: &DirEntry, counts: bool, s: &mut dyn Streams) -> bool {
        let nstrips = self.dir.nstrips;
        self.check_dir_count(dp, nstrips, s);
        let have = if counts {
            self.dir.stripbytecount.is_some()
        } else {
            self.dir.stripoffset.is_some()
        };
        if !have && !self.check_malloc(u64::from(nstrips) * 4, "for strip array", s) {
            return false;
        }
        // The array is set (zeroed) before the values are fetched, and stays when that fails.
        let zeroed = vec![0u32; nstrips as usize];
        if counts {
            self.dir.stripbytecount = Some(zeroed);
        } else {
            self.dir.stripoffset = Some(zeroed);
        }
        let mut lp = vec![0u32; nstrips as usize];
        let status;
        // `i < nstrips` (a long) and `i < (int) dir->tdir_count`.
        let limit = i64::from(nstrips).min(i64::from(dp.count as i32)).max(0) as usize;
        if dp.ty == fields::TIFF_SHORT {
            if !self.check_malloc(u64::from(dp.count) * 2, "to fetch strip tag", s) {
                return false;
            }
            match self.fetch_short_array(dp, s) {
                Some(v) => {
                    for i in 0..limit {
                        lp[i] = u32::from(v[i]);
                    }
                    status = true;
                }
                None => status = false,
            }
        } else if i64::from(nstrips) != i64::from(dp.count as i32) {
            if !self.check_malloc(u64::from(dp.count) * 4, "to fetch strip tag", s) {
                return false;
            }
            match self.fetch_long_array(dp, s) {
                Some(v) => {
                    for i in 0..limit {
                        lp[i] = v.get(i).copied().unwrap_or(0);
                    }
                    status = true;
                }
                None => status = false,
            }
        } else {
            match self.fetch_long_array(dp, s) {
                Some(v) => {
                    let n = v.len().min(lp.len());
                    lp[..n].copy_from_slice(&v[..n]);
                    status = true;
                }
                None => status = false,
            }
        }
        // The array stays (zeroed, partly filled) even when the fetch fails.
        if counts {
            self.dir.stripbytecount = Some(lp);
        } else {
            self.dir.stripoffset = Some(lp);
        }
        status
    }

    /// `TIFFFetchExtraSamples`.
    fn fetch_extra_samples(&mut self, dp: &DirEntry, s: &mut dyn Streams) -> bool {
        let v = if dp.ty == fields::TIFF_BYTE {
            let v = self.fetch_byte_array(dp, s);
            if v.is_some() && !self.fix() {
                self.byte_array_notice(dp);
            }
            v
        } else {
            self.fetch_short_array(dp, s)
        };
        match v {
            Some(v) => {
                self.set_field_arg(u32::from(dp.tag), Arg::Count(dp.count, Data::U16(&v)), s)
            }
            None => false,
        }
    }

    /// The colour map and the transfer function (the second pass's case).
    fn fetch_colormap(&mut self, dp: &DirEntry, s: &mut dyn Streams) {
        let v = 1u64 << (self.dir.bitspersample & 63);
        let v32 = v as u32;
        if (u32::from(dp.tag) == tag::COLORMAP || dp.count != v32)
            && !self.check_dir_count(dp, (3 * v) as u32, s)
        {
            return;
        }
        if !self.check_malloc(
            u64::from(dp.count) * 2,
            "to read \"TransferFunction\" tag",
            s,
        ) {
            return;
        }
        if self.fetch_data(dp, s).is_some() {
            self.dir.set_bit(if u32::from(dp.tag) == tag::COLORMAP {
                bit::COLORMAP
            } else {
                bit::TRANSFERFUNCTION
            });
            self.flags |= super::TIFF_DIRTYDIRECT;
        }
    }

    /// `TIFFFetchShortPair`.
    fn fetch_short_pair(&mut self, dp: &DirEntry, s: &mut dyn Streams) -> bool {
        let v = match dp.ty {
            fields::TIFF_SHORT | fields::TIFF_SSHORT => self.fetch_short_array(dp, s),
            fields::TIFF_BYTE | fields::TIFF_SBYTE => self.fetch_byte_array(dp, s),
            _ => None,
        };
        match v {
            Some(v) => {
                let a = v.first().copied().unwrap_or(0);
                let b = v.get(1).copied().unwrap_or(0);
                self.set_field_arg(
                    u32::from(dp.tag),
                    Arg::Int2(u32::from(a), u32::from(b)),
                    s,
                );
                true
            }
            None => false,
        }
    }

    /// `TIFFFetchRefBlackWhite`.
    fn fetch_ref_black_white(&mut self, dp: &DirEntry, s: &mut dyn Streams) -> bool {
        if dp.ty == fields::TIFF_RATIONAL {
            return self.fetch_normal_tag(dp, s);
        }
        const MESG: &str = "for \"ReferenceBlackWhite\" array";
        if !self.check_malloc(u64::from(dp.count) * 4, MESG, s) {
            return false;
        }
        let Some(l) = self.fetch_long_array(dp, s) else {
            return false;
        };
        if !self.check_malloc(u64::from(dp.count) * 4, MESG, s) {
            return false;
        }
        let f: Vec<f32> = l.iter().map(|&x| x as f32).collect();
        self.set_field_arg(u32::from(dp.tag), Arg::Ptr(Data::F32(&f)), s)
    }

    /// `EstimateStripByteCounts(tif, dir, dircount)`.
    fn estimate_strip_byte_counts(&mut self, dir: &[DirEntry], s: &mut dyn Streams) -> bool {
        const M: &str = "EstimateStripByteCounts";
        let nstrips = self.dir.nstrips;
        self.dir.stripbytecount = None;
        let ok = self.check_malloc(
            u64::from(nstrips) * 4,
            "for \"StripByteCounts\" array",
            s,
        );
        let mut counts = vec![0u32; if ok { nstrips as usize } else { 0 }];
        if self.dir.compression != compression::NONE {
            let mut space: u32 = (8 + 2 + dir.len() * 12 + 4) as u32;
            let filesize = self.fd.size();
            for dp in dir {
                let cc = fields::data_width(dp.ty) as u32;
                if cc == 0 {
                    let m = format!(
                        "{}: Cannot determine size of unknown tag type {}",
                        cut(&self.name, 1000),
                        dp.ty
                    );
                    self.error(Some(M), &m, s);
                    return false;
                }
                let cc = cc.wrapping_mul(dp.count);
                if cc > 4 {
                    space = space.wrapping_add(cc);
                }
            }
            space = filesize.wrapping_sub(space);
            if self.dir.planarconfig == values::PLANARCONFIG_SEPARATE {
                space /= u32::from(self.dir.samplesperpixel);
            }
            for c in counts.iter_mut() {
                *c = space;
            }
            if nstrips == 0 {
                self.notices.add(
                    &FS_821,
                    format!(
                        "{}: no strips to estimate: libtiff 3.6.1 trims the strip at index 65535 \
                         of its empty arrays (memory outside them); nothing trimmed",
                        self.name
                    ),
                );
            } else if let Some(&off) = self
                .dir
                .stripoffset
                .as_ref()
                .and_then(|v| v.get(nstrips as usize - 1))
            {
                let i = nstrips as usize - 1;
                if let Some(c) = counts.get_mut(i) {
                    if off.wrapping_add(*c) > filesize {
                        *c = filesize.wrapping_sub(off);
                    }
                }
            }
        } else {
            let rowbytes = self.scanline_size() as u32;
            if self.dir.stripsperimage == 0 {
                self.notices.add(
                    &FS_821,
                    format!(
                        "{}: no strips to estimate: libtiff 3.6.1 divides by zero strips per \
                         image (a crash); nothing estimated",
                        self.name
                    ),
                );
            } else {
                let rowsperstrip = self.dir.imagelength / self.dir.stripsperimage;
                for c in counts.iter_mut() {
                    *c = rowbytes.wrapping_mul(rowsperstrip);
                }
            }
        }
        if ok {
            self.dir.stripbytecount = Some(counts);
        }
        self.dir.set_bit(bit::STRIPBYTECOUNTS);
        if !self.dir.is_set(bit::ROWSPERSTRIP) {
            self.dir.rowsperstrip = self.dir.imagelength;
        }
        true
    }

    /// `ChopUpSingleUncompressedStrip(tif)`.
    fn chop_up_single_uncompressed_strip(&mut self, s: &mut dyn Streams) {
        let mut bytecount = self.dir.stripbytecount.as_ref().map(|v| v[0]).unwrap_or(0);
        let mut offset = self.dir.stripoffset.as_ref().map(|v| v[0]).unwrap_or(0);
        let rowbytes = self.vtile_size(1);
        let (mut stripbytes, rowsperstrip): (i32, u32);
        if rowbytes > 8192 {
            stripbytes = rowbytes;
            rowsperstrip = 1;
        } else if rowbytes > 0 {
            rowsperstrip = (8192 / rowbytes) as u32;
            stripbytes = rowbytes.wrapping_mul(rowsperstrip as i32);
        } else {
            return;
        }
        if rowsperstrip >= self.dir.rowsperstrip {
            return;
        }
        let sb = stripbytes as u32;
        let nstrips = bytecount.wrapping_add(sb.wrapping_sub(1)) / sb;
        let a = self.check_malloc(
            u64::from(nstrips) * 4,
            "for chopped \"StripByteCounts\" array",
            s,
        );
        let b = self.check_malloc(
            u64::from(nstrips) * 4,
            "for chopped \"StripOffsets\" array",
            s,
        );
        if !a || !b {
            return;
        }
        let mut newcounts = vec![0u32; nstrips as usize];
        let mut newoffsets = vec![0u32; nstrips as usize];
        for st in 0..nstrips as usize {
            if stripbytes > bytecount as i32 {
                stripbytes = bytecount as i32;
            }
            newcounts[st] = stripbytes as u32;
            newoffsets[st] = offset;
            offset = offset.wrapping_add(stripbytes as u32);
            bytecount = bytecount.wrapping_sub(stripbytes as u32);
        }
        self.dir.nstrips = nstrips;
        self.dir.stripsperimage = nstrips;
        self.set_field_arg(tag::ROWSPERSTRIP, Arg::Int(rowsperstrip), s);
        self.dir.stripbytecount = Some(newcounts);
        self.dir.stripoffset = Some(newoffsets);
    }
}

#[allow(dead_code)]
fn _unused(_: FieldRef, _: Whence) {}
