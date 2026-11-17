// SPDX-License-Identifier: Apache-2.0
//! The voxel arithmetic (spec §6-§10): clones, clearing, size checks and frame copies, value
//! ranges, scaling, thresholds, binarising and signs, and the functions of two volumes; FS-289
//! to FS-292 and FS-296.

use larmorx_core::defects::{FS_291, FS_292, FS_296, FS_332};

use super::{Dst, VolOut, Vols, direct_place};
use crate::mri::voxels::Place;
use crate::mri::{Mri, Region, types};
use crate::process::{Process, Stop, codes};
use crate::sse;

/// The bytes of one element of a direct walk of type `kind` (§6): the integer and float types.
pub fn elem_size(kind: i32) -> Option<u64> {
    match kind {
        types::UCHAR => Some(1),
        types::SHORT | types::USHRT => Some(2),
        types::INT | types::FLOAT => Some(4),
        types::LONG => Some(8),
        _ => None,
    }
}

/// An element of a direct walk, by its type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Elem {
    U8(u8),
    I16(i16),
    U16(u16),
    I32(i32),
    I64(i64),
    F32(f32),
}

impl Elem {
    /// The element of type `kind` in its little-endian bytes.
    pub fn decode(kind: i32, b: &[u8]) -> Elem {
        match kind {
            types::UCHAR => Elem::U8(b[0]),
            types::SHORT => Elem::I16(i16::from_le_bytes([b[0], b[1]])),
            types::USHRT => Elem::U16(u16::from_le_bytes([b[0], b[1]])),
            types::INT => Elem::I32(i32::from_le_bytes(b[..4].try_into().expect("4"))),
            types::LONG => Elem::I64(i64::from_le_bytes(b[..8].try_into().expect("8"))),
            _ => Elem::F32(f32::from_le_bytes(b[..4].try_into().expect("4"))),
        }
    }

    /// The element converted to float (C's conversion).
    pub fn to_f32(self) -> f32 {
        match self {
            Elem::U8(v) => f32::from(v),
            Elem::I16(v) => f32::from(v),
            Elem::U16(v) => f32::from(v),
            Elem::I32(v) => v as f32,
            Elem::I64(v) => v as f32,
            Elem::F32(v) => v,
        }
    }

    /// The element as an integer (C's integer promotion), for the integer types.
    fn to_i64(self) -> i64 {
        match self {
            Elem::U8(v) => i64::from(v),
            Elem::I16(v) => i64::from(v),
            Elem::U16(v) => i64::from(v),
            Elem::I32(v) => i64::from(v),
            Elem::I64(v) => v,
            Elem::F32(v) => v as i64,
        }
    }
}

/// An integer's low bits as an element of the integer type `kind`.
fn int_bytes(kind: i32, v: i64) -> Vec<u8> {
    match kind {
        types::UCHAR => vec![v as u8],
        types::SHORT => (v as i16).to_le_bytes().to_vec(),
        types::USHRT => (v as u16).to_le_bytes().to_vec(),
        types::INT => (v as i32).to_le_bytes().to_vec(),
        _ => v.to_le_bytes().to_vec(),
    }
}

/// C's `if (x < lo) x = lo; if (x > hi) x = hi;` (a NaN stays).
fn clip(x: f32, lo: f32, hi: f32) -> f32 {
    let mut x = x;
    if x < lo {
        x = lo;
    }
    if x > hi {
        x = hi;
    }
    x
}

impl Process {
    /// A direct element read (§6) of type `kind` at (x, y, z, f) of `m`: FS-58 without voxels,
    /// FS-289 and FS-290 outside the volume.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn direct_get(
        &mut self,
        m: &Mri,
        kind: i32,
        x: i64,
        y: i64,
        z: i64,
        f: i64,
        what: &str,
    ) -> Result<Elem, Stop> {
        self.direct_needs_voxels(m, what)?;
        let n = elem_size(kind).expect("a direct type");
        let place = direct_place(m, x, y, z, f, n);
        let b = self.direct_read(m, place, n as usize, what);
        Ok(Elem::decode(kind, &b))
    }

    /// A direct element write (§6) of bytes at (x, y, z, f) of `m`, elements of `n` bytes.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn direct_put(
        &mut self,
        m: &mut Mri,
        x: i64,
        y: i64,
        z: i64,
        f: i64,
        b: &[u8],
        what: &str,
    ) -> Result<(), Stop> {
        self.direct_needs_voxels(m, what)?;
        let place = direct_place(m, x, y, z, f, b.len() as u64);
        self.direct_write(m, place, b, what);
        Ok(())
    }

    /// `MRIclone(src, NULL)` (§6): a new volume of the source's extent, type and frames, the
    /// source's colour table (deep copied) and header.
    pub fn mri_clone_new(&mut self, src: &Mri) -> Result<Mri, Stop> {
        let g = &src.geom;
        let mut d = self.mri_alloc_sequence(g.width, g.height, g.depth, src.kind, src.nframes)?;
        if let Some(ct) = &src.ct {
            d.ct = self.ctab_deep_copy(Some(ct));
        }
        self.copy_header_into(src, &mut d)?;
        Ok(d)
    }

    /// A new volume of the source's extent with `kind` and `nframes`, the source's header.
    fn new_with_header(&mut self, src: &Mri, kind: i32, nframes: i32) -> Result<Mri, Stop> {
        let g = &src.geom;
        let mut d = self.mri_alloc_sequence(g.width, g.height, g.depth, kind, nframes)?;
        self.copy_header_into(src, &mut d)?;
        Ok(d)
    }

    /// The destination of a function that maps the first source (§9: a clone without one).
    fn clone_dst(&mut self, v: &mut Vols) -> Result<(), Stop> {
        if !v.has_dst() {
            let c = self.mri_clone_new(v.a())?;
            v.set_dst(c);
        }
        Ok(())
    }

    // ---- §7

    /// `MRIclone(src, dst)` (§7): a given destination takes the colour table (when the source
    /// has one) and the header; else a clone.
    pub fn mri_clone(&mut self, src: &mut Mri, dst: Dst) -> Result<VolOut, Stop> {
        match dst {
            Dst::New | Dst::Other => Ok(VolOut::New(self.mri_clone_new(src)?)),
            Dst::Src => {
                // FS-332: FreeSurfer frees the destination's colour table (the source's own)
                // before copying the source's: the copy of no table fails and the volume keeps
                // none (`reproduce`); by default the volume keeps its table.
                if src.ct.is_some() {
                    if self.mode().reproduce() {
                        self.note(
                            &FS_332,
                            "a volume cloned into itself lost its colour table (FreeSurfer frees it before copying it)",
                        );
                        src.ct = None;
                        self.error_printf(codes::BAD_PARM, b"CTABdeepCopy: ct was NULL");
                    } else {
                        self.note(&FS_332, "a volume cloned into itself kept its colour table");
                    }
                }
                let s = src.clone();
                if let Some(ct) = &s.ct {
                    src.ct = self.ctab_deep_copy(Some(ct));
                }
                self.copy_header_into(&s, src)?;
                Ok(VolOut::Src)
            }
            Dst::Given(d) => {
                if let Some(ct) = &src.ct {
                    d.ct = self.ctab_deep_copy(Some(ct));
                }
                self.copy_header_into(src, d)?;
                Ok(VolOut::Given)
            }
        }
    }

    /// `MRIcloneBySpace(src, type, nframes)` (§7): a new volume of the source's extent, `type`
    /// and `nframes` (the source's when negative), its header and pulse parameters.
    pub fn mri_clone_by_space(&mut self, src: &Mri, kind: i32, nframes: i32) -> Result<Mri, Stop> {
        let kind = if kind < 0 { src.kind } else { kind };
        let nframes = if nframes < 0 { src.nframes } else { nframes };
        let mut d = self.new_with_header(src, kind, nframes)?;
        self.copy_pulse_parameters(src, &mut d);
        Ok(d)
    }

    /// `MRIclear(mri)` (§7): every voxel's bytes 0; another type: `ERROR_UNSUPPORTED`.
    pub fn mri_clear(&mut self, m: &mut Mri) -> Result<i32, Stop> {
        let cleared = matches!(
            m.kind,
            types::UCHAR
                | types::SHORT
                | types::USHRT
                | types::INT
                | types::LONG
                | types::FLOAT
                | types::BITMAP
        );
        if !cleared {
            self.error_printf(
                codes::UNSUPPORTED,
                format!("MRIclear: unsupported input type {}", m.kind).as_bytes(),
            );
            return Ok(codes::UNSUPPORTED);
        }
        let g = &m.geom;
        let rows =
            i64::from(g.height).max(0) * i64::from(g.depth).max(0) * i64::from(m.nframes).max(0);
        if rows > 0 {
            self.direct_needs_voxels(m, "MRIclear")?;
            if let Some(v) = m.voxels.as_mut() {
                v.fill(0);
            }
        }
        Ok(0)
    }

    /// `MRImatch(a, b)` (§7): 1 when the extents and types are equal.
    pub fn mri_match(&mut self, a: &Mri, b: &Mri) -> i32 {
        let (g, h) = (&a.geom, &b.geom);
        i32::from(
            g.width == h.width && g.height == h.height && g.depth == h.depth && a.kind == b.kind,
        )
    }

    /// `MRImatchDimensions(a, b)` (§7): 1 when the extents are equal and each voxel size differs
    /// by less than `FLT_EPSILON`.
    pub fn mri_match_dimensions(&mut self, a: &Mri, b: &Mri) -> i32 {
        let (g, h) = (&a.geom, &b.geom);
        if g.width != h.width || g.height != h.height || g.depth != h.depth {
            return 0;
        }
        let close = |x: f32, y: f32| sse::sub32(x, y).abs() < f32::EPSILON;
        i32::from(close(g.xsize, h.xsize) && close(g.ysize, h.ysize) && close(g.zsize, h.zsize))
    }

    /// `MRIcheckSize(src, check, w, h, d)` (§7): 0 without `check`, else 1 when its extent is
    /// `w x h x d` (a 0 taken from the source).
    pub fn mri_check_size(
        &mut self,
        src: &Mri,
        check: Option<&Mri>,
        w: i32,
        h: i32,
        d: i32,
    ) -> i32 {
        let Some(c) = check else { return 0 };
        let w = if w == 0 { src.geom.width } else { w };
        let h = if h == 0 { src.geom.height } else { h };
        let d = if d == 0 { src.geom.depth } else { d };
        i32::from(c.geom.width == w && c.geom.height == h && c.geom.depth == d)
    }

    /// `MRIcopyFrame(src, dst, sf, df)` (§7).
    pub fn mri_copy_frame(
        &mut self,
        src: &mut Mri,
        dst: Dst,
        sf: i32,
        df: i32,
    ) -> Result<VolOut, Stop> {
        let mut v = Vols::new(src, None, dst);
        if !v.has_dst() {
            let mut d = self.new_with_header(v.a(), v.a().kind, df.wrapping_add(1))?;
            self.copy_pulse_parameters(v.a(), &mut d);
            v.set_dst(d);
        }
        if !self.copy_frame_into(&mut v, sf, df)? {
            return Ok(VolOut::Nothing);
        }
        Ok(v.out())
    }

    /// The copy of frame `sf` of the first volume into frame `df` of the destination: false
    /// after the out-of-range error.
    fn copy_frame_into(&mut self, v: &mut Vols, sf: i32, df: i32) -> Result<bool, Stop> {
        let n = v.d().nframes;
        if df >= n {
            self.error_printf(
                codes::BAD_PARM,
                format!("MRIcopyFrame: dst frame #{df} out of range (nframes={n})\n").as_bytes(),
            );
            return Ok(false);
        }
        let g = v.a().geom.clone();
        for z in 0..g.depth {
            for y in 0..g.height {
                for x in 0..g.width {
                    let val = self.mri_get_vox_val(v.a(), x, y, z, sf, 0)?;
                    self.mri_set_vox_val(v.d_mut(), x, y, z, df, val, 0)?;
                }
            }
        }
        Ok(true)
    }

    /// `MRIcopyFrames(src, dst, s0, s1, d0)` (§7).
    pub fn mri_copy_frames(
        &mut self,
        src: &mut Mri,
        dst: Dst,
        s0: i32,
        s1: i32,
        d0: i32,
    ) -> Result<VolOut, Stop> {
        let mut v = Vols::new(src, None, dst);
        if !v.has_dst() {
            let n = s1.wrapping_sub(s0).wrapping_add(1);
            let d = self.new_with_header(v.a(), v.a().kind, n)?;
            v.set_dst(d);
        }
        if self.mri_match(v.a(), v.d()) == 0 {
            let (a, d) = (&v.a().geom, &v.d().geom);
            let msg = format!(
                "MRIcopyFrames({s0}, {s1}, {d0}): src ({}, {}, {}) doesn't match destimation ({}, {}, {})\n",
                a.width, a.height, a.depth, d.width, d.height, d.depth
            );
            self.error_exit(codes::BAD_PARM, msg.as_bytes())?;
        }
        let mut f = s0;
        while f <= s1 {
            if !self.copy_frame_into(&mut v, f, f - s0 + d0)? {
                return Ok(VolOut::Nothing);
            }
            f += 1;
        }
        Ok(v.out())
    }

    // ---- §8

    /// `MRIvalRange(mri, &min, &max)` (§8): the range over every frame; returns 0.
    pub fn mri_val_range(&mut self, m: &Mri, lo: &mut f32, hi: &mut f32) -> Result<i32, Stop> {
        let (mut a, mut b) = (10000.0f32, -10000.0f32);
        let g = &m.geom;
        for f in 0..m.nframes {
            for z in 0..g.depth {
                for y in 0..g.height {
                    for x in 0..g.width {
                        let val = self.mri_get_vox_val(m, x, y, z, f, 0)?;
                        if val < a {
                            a = val;
                        }
                        if val > b {
                            b = val;
                        }
                    }
                }
            }
        }
        *lo = a;
        *hi = b;
        Ok(0)
    }

    /// `MRIvalRangeFrame(mri, &min, &max, frame)` (§8): one frame, read directly (float, int,
    /// short, ushort, uchar); another type: the error and `ERROR_UNSUPPORTED`, the range not set.
    pub fn mri_val_range_frame(
        &mut self,
        m: &Mri,
        lo: &mut f32,
        hi: &mut f32,
        frame: i32,
    ) -> Result<i32, Stop> {
        if !matches!(
            m.kind,
            types::FLOAT | types::INT | types::SHORT | types::USHRT | types::UCHAR
        ) {
            self.error_printf(
                codes::UNSUPPORTED,
                format!("MRIvalRange: unsupported type {}", m.kind).as_bytes(),
            );
            return Ok(codes::UNSUPPORTED);
        }
        let (mut a, mut b) = (10000.0f32, -10000.0f32);
        let g = m.geom.clone();
        for z in 0..g.depth {
            for y in 0..g.height {
                for x in 0..g.width {
                    let e = self.direct_get(
                        m,
                        m.kind,
                        i64::from(x),
                        i64::from(y),
                        i64::from(z),
                        i64::from(frame),
                        "MRIvalRangeFrame",
                    )?;
                    let val = e.to_f32();
                    if val < a {
                        a = val;
                    }
                    if val > b {
                        b = val;
                    }
                }
            }
        }
        *lo = a;
        *hi = b;
        Ok(0)
    }

    /// `MRIvalRangeRegion(mri, &min, &max, region)` (§8): frame 0 over the region, its start
    /// raised to 0 and its end lowered to the extent (no region: the whole volume; FreeSurfer
    /// needs one); returns 0.
    pub fn mri_val_range_region(
        &mut self,
        m: &Mri,
        lo: &mut f32,
        hi: &mut f32,
        region: Option<&Region>,
    ) -> Result<i32, Stop> {
        let g = &m.geom;
        let whole = Region {
            x: 0,
            y: 0,
            z: 0,
            dx: g.width,
            dy: g.height,
            dz: g.depth,
        };
        let r = region.unwrap_or(&whole);
        let span = |s: i32, d: i32, e: i32| (s.max(0), s.saturating_add(d).min(e));
        let (x0, x1) = span(r.x, r.dx, g.width);
        let (y0, y1) = span(r.y, r.dy, g.height);
        let (z0, z1) = span(r.z, r.dz, g.depth);
        let (mut a, mut b) = (10000.0f32, -10000.0f32);
        for z in z0..z1 {
            for y in y0..y1 {
                for x in x0..x1 {
                    let val = self.mri_get_vox_val(m, x, y, z, 0, 0)?;
                    if val < a {
                        a = val;
                    }
                    if val > b {
                        b = val;
                    }
                }
            }
        }
        *lo = a;
        *hi = b;
        Ok(0)
    }

    /// `MRIlabelValRange(mri, labeled, label, &min, &max)` (§8): the range of the voxels whose
    /// label (`nint` of the labelled volume's value) is `label`; returns 0.
    pub fn mri_label_val_range(
        &mut self,
        m: &Mri,
        labeled: &Mri,
        label: i32,
        lo: &mut f32,
        hi: &mut f32,
    ) -> Result<i32, Stop> {
        let (mut a, mut b) = (10000.0f32, -10000.0f32);
        let mut first = true;
        let g = &m.geom;
        for f in 0..m.nframes {
            for z in 0..g.depth {
                for y in 0..g.height {
                    for x in 0..g.width {
                        let l = self.mri_get_vox_val(labeled, x, y, z, f, 0)?;
                        if self.nint(sse::widen(l)) != label {
                            continue;
                        }
                        let val = self.mri_get_vox_val(m, x, y, z, f, 0)?;
                        if first {
                            a = val;
                            b = val;
                            first = false;
                        }
                        if val < a {
                            a = val;
                        }
                        if val > b {
                            b = val;
                        }
                    }
                }
            }
        }
        *lo = a;
        *hi = b;
        Ok(0)
    }

    // ---- §9

    /// Each voxel of each frame of the first volume's extent mapped into the destination (a
    /// clone without one) through `map` (`None`: the voxel left as it is).
    fn map_voxels(
        &mut self,
        src: &mut Mri,
        dst: Dst,
        map: &mut dyn FnMut(&mut Process, f32) -> Option<f32>,
    ) -> Result<VolOut, Stop> {
        let mut v = Vols::new(src, None, dst);
        self.clone_dst(&mut v)?;
        let (g, nf) = (v.a().geom.clone(), v.a().nframes);
        for f in 0..nf {
            for z in 0..g.depth {
                for y in 0..g.height {
                    for x in 0..g.width {
                        let val = self.mri_get_vox_val(v.a(), x, y, z, f, 0)?;
                        if let Some(o) = map(self, val) {
                            self.mri_set_vox_val(v.d_mut(), x, y, z, f, o, 0)?;
                        }
                    }
                }
            }
        }
        Ok(v.out())
    }

    /// `MRIlinearScale(src, dst, scale, offset, only_nonzero)` (§9).
    pub fn mri_linear_scale(
        &mut self,
        src: &mut Mri,
        dst: Dst,
        scale: f32,
        offset: f32,
        only_nonzero: i32,
    ) -> Result<VolOut, Stop> {
        let uchar = match &dst {
            Dst::Given(d) => d.kind == types::UCHAR,
            _ => src.kind == types::UCHAR,
        };
        self.map_voxels(src, dst, &mut |_, val| {
            // with `only_nonzero` the other voxels get their own value (observed: the probe's `vscale`)
            if only_nonzero != 0 && f64::from(val.abs()) < 1e-15 {
                return Some(val);
            }
            let mut o = sse::add32(sse::mul32(val, scale), offset);
            if uchar {
                o = clip(o, 0.0, 255.0);
            }
            Some(o)
        })
    }

    /// `MRIscalarMul(src, dst, s)` (§9): `v * s`.
    pub fn mri_scalar_mul(&mut self, src: &mut Mri, dst: Dst, s: f32) -> Result<VolOut, Stop> {
        self.map_voxels(src, dst, &mut |_, val| Some(sse::mul32(val, s)))
    }

    /// `MRIscalarMulFrame(src, dst, s, frame)` (§9): one frame, read and written directly by the
    /// source's type (FS-296: a NaN product stored into a byte as 0).
    pub fn mri_scalar_mul_frame(
        &mut self,
        src: &mut Mri,
        dst: Dst,
        s: f32,
        frame: i32,
    ) -> Result<VolOut, Stop> {
        let mut v = Vols::new(src, None, dst);
        self.clone_dst(&mut v)?;
        let (g, kind) = (v.a().geom.clone(), v.a().kind);
        let what = "MRIscalarMulFrame";
        let f = i64::from(frame);
        for z in 0..i64::from(g.depth) {
            for y in 0..i64::from(g.height) {
                for x in 0..i64::from(g.width) {
                    let bytes = match kind {
                        types::UCHAR => {
                            let e = self.direct_get(v.a(), kind, x, y, z, f, what)?;
                            let p = clip(sse::mul32(e.to_f32(), s), 0.0, 255.0);
                            if p.is_nan() {
                                self.note(
                                    &FS_296,
                                    "a NaN product stored into a byte voxel as 0 (undefined in C)",
                                );
                                vec![0]
                            } else {
                                vec![p as u8]
                            }
                        }
                        types::FLOAT => {
                            let e = self.direct_get(v.a(), kind, x, y, z, f, what)?;
                            sse::mul32(e.to_f32(), s).to_le_bytes().to_vec()
                        }
                        types::SHORT | types::USHRT => {
                            let e = self.direct_get(v.a(), kind, x, y, z, f, what)?;
                            let i = self.nint(sse::widen(sse::mul32(e.to_f32(), s)));
                            int_bytes(kind, i64::from(i))
                        }
                        _ => {
                            self.error_printf(
                                codes::UNSUPPORTED,
                                format!("MRIscalarMulFrame: unsupported type {kind}").as_bytes(),
                            );
                            return Ok(VolOut::Nothing);
                        }
                    };
                    self.direct_put(v.d_mut(), x, y, z, f, &bytes, what)?;
                }
            }
        }
        Ok(v.out())
    }

    /// `MRIthreshold(src, dst, t)` (§9): `v < t` becomes 0.
    pub fn mri_threshold(&mut self, src: &mut Mri, dst: Dst, t: f32) -> Result<VolOut, Stop> {
        self.map_voxels(src, dst, &mut |_, val| {
            Some(if val < t { 0.0 } else { val })
        })
    }

    /// `MRIupperthresholdAllFrames(src, dst, t)` (§9): a voxel above `t` in any frame becomes 0
    /// in every frame; the others copied.
    pub fn mri_upper_threshold_all_frames(
        &mut self,
        src: &mut Mri,
        dst: Dst,
        t: f32,
    ) -> Result<VolOut, Stop> {
        let mut v = Vols::new(src, None, dst);
        self.clone_dst(&mut v)?;
        let (g, nf) = (v.a().geom.clone(), v.a().nframes);
        for z in 0..g.depth {
            for y in 0..g.height {
                for x in 0..g.width {
                    let mut above = false;
                    for f in 0..nf {
                        if self.mri_get_vox_val(v.a(), x, y, z, f, 0)? > t {
                            above = true;
                            break;
                        }
                    }
                    for f in 0..nf {
                        let val = if above {
                            0.0
                        } else {
                            self.mri_get_vox_val(v.a(), x, y, z, f, 0)?
                        };
                        self.mri_set_vox_val(v.d_mut(), x, y, z, f, val, 0)?;
                    }
                }
            }
        }
        Ok(v.out())
    }

    /// `MRIbinarize(src, dst, t, low, high)` (§9): `low` where `v < t` (in double), else `high`.
    pub fn mri_binarize(
        &mut self,
        src: &mut Mri,
        dst: Dst,
        t: f32,
        low: f32,
        high: f32,
    ) -> Result<VolOut, Stop> {
        self.map_voxels(src, dst, &mut |_, val| {
            Some(if f64::from(val) < f64::from(t) {
                low
            } else {
                high
            })
        })
    }

    /// FS-291: a byte function on a volume of another type. `reproduce`: the rows' first bytes
    /// taken as byte voxels (true); by default the end of the program.
    fn bytes_only(&mut self, name: &str, kind: i32) -> Result<(), Stop> {
        if kind == types::UCHAR {
            return Ok(());
        }
        if self.mode().reproduce() {
            self.note(
                &FS_291,
                format!(
                    "{name} on a volume of type {kind}: its rows' first bytes taken as byte voxels"
                ),
            );
            return Ok(());
        }
        self.note(
            &FS_291,
            format!("{name} on a volume of type {kind} refused (byte volumes only)"),
        );
        self.error_exit(
            codes::UNSUPPORTED,
            format!("{name}: unsupported type {kind} (uchar volumes only)").as_bytes(),
        )
    }

    /// `MRIinvertContrast(src, dst, t)` (§9): frame 0, byte voxels (direct): `v > t` becomes
    /// `255 - v`, the others copied (FS-291).
    pub fn mri_invert_contrast(&mut self, src: &mut Mri, dst: Dst, t: f32) -> Result<VolOut, Stop> {
        let mut v = Vols::new(src, None, dst);
        self.clone_dst(&mut v)?;
        self.bytes_only("MRIinvertContrast", v.a().kind)?;
        if let Some(k) = Some(v.d().kind).filter(|&k| k != v.a().kind) {
            self.bytes_only("MRIinvertContrast", k)?;
        }
        let g = v.a().geom.clone();
        let what = "MRIinvertContrast";
        for z in 0..i64::from(g.depth) {
            for y in 0..i64::from(g.height) {
                for x in 0..i64::from(g.width) {
                    let e = self.direct_get(v.a(), types::UCHAR, x, y, z, 0, what)?;
                    let Elem::U8(b) = e else { unreachable!() };
                    let o = if f32::from(b) > t { 255 - b } else { b };
                    self.direct_put(v.d_mut(), x, y, z, 0, &[o], what)?;
                }
            }
        }
        Ok(v.out())
    }

    /// `MRIabs`, `MRIpos`, `MRIneg` (§9; `which` 0, 1, 2): without a destination a new volume
    /// with the source's header; frames innermost.
    pub fn mri_sign(&mut self, src: &mut Mri, dst: Dst, which: u8) -> Result<VolOut, Stop> {
        let mut v = Vols::new(src, None, dst);
        if !v.has_dst() {
            let (kind, nf) = (v.a().kind, v.a().nframes);
            let d = self.new_with_header(v.a(), kind, nf)?;
            v.set_dst(d);
        }
        let (g, nf) = (v.a().geom.clone(), v.a().nframes);
        for z in 0..g.depth {
            for y in 0..g.height {
                for x in 0..g.width {
                    for f in 0..nf {
                        let val = self.mri_get_vox_val(v.a(), x, y, z, f, 0)?;
                        let o = match which {
                            0 => val.abs(),
                            1 => {
                                if val < 0.0 {
                                    0.0
                                } else {
                                    val
                                }
                            }
                            _ => {
                                if val > 0.0 {
                                    0.0
                                } else {
                                    val
                                }
                            }
                        };
                        self.mri_set_vox_val(v.d_mut(), x, y, z, f, o, 0)?;
                    }
                }
            }
        }
        Ok(v.out())
    }

    // ---- §10

    /// `MRImultiply(a, b, dst)` (§10): frame 0, `va * vb`.
    pub fn mri_multiply(&mut self, a: &mut Mri, b: &mut Mri, dst: Dst) -> Result<VolOut, Stop> {
        let mut v = Vols::new(a, Some(b), dst);
        if !v.has_dst() {
            let d = self.new_with_header(v.a(), v.a().kind, 1)?;
            v.set_dst(d);
        }
        let g = v.a().geom.clone();
        for z in 0..g.depth {
            for y in 0..g.height {
                for x in 0..g.width {
                    let va = self.mri_get_vox_val(v.a(), x, y, z, 0, 0)?;
                    let vb = self.mri_get_vox_val(v.b(), x, y, z, 0, 0)?;
                    self.mri_set_vox_val(v.d_mut(), x, y, z, 0, sse::mul32(va, vb), 0)?;
                }
            }
        }
        Ok(v.out())
    }

    /// The destination of `MRIsubtract` and `MRIadd`: the first volume's frames (1 when 0).
    fn two_dst(&mut self, v: &mut Vols) -> Result<i32, Stop> {
        let nf = if v.a().nframes == 0 { 1 } else { v.a().nframes };
        if !v.has_dst() {
            let d = self.new_with_header(v.a(), v.a().kind, nf)?;
            v.set_dst(d);
        }
        Ok(nf)
    }

    /// `MRIsubtract(a, b, dst)` (§10).
    pub fn mri_subtract(&mut self, a: &mut Mri, b: &mut Mri, dst: Dst) -> Result<VolOut, Stop> {
        self.add_or_subtract(a, b, dst, false)
    }

    /// `MRIadd(a, b, dst)` (§10).
    pub fn mri_add(&mut self, a: &mut Mri, b: &mut Mri, dst: Dst) -> Result<VolOut, Stop> {
        self.add_or_subtract(a, b, dst, true)
    }

    fn add_or_subtract(
        &mut self,
        a: &mut Mri,
        b: &mut Mri,
        dst: Dst,
        add: bool,
    ) -> Result<VolOut, Stop> {
        let mut v = Vols::new(a, Some(b), dst);
        let nf = self.two_dst(&mut v)?;
        let g = v.a().geom.clone();
        let (ka, kb, kd) = (v.a().kind, v.b().kind, v.d().kind);
        let what = if add { "MRIadd" } else { "MRIsubtract" };
        if ka != kb || (add && ka == types::UCHAR) {
            for f in 0..nf {
                for z in 0..g.depth {
                    for y in 0..g.height {
                        for x in 0..g.width {
                            let va = self.mri_get_vox_val(v.a(), x, y, z, f, 0)?;
                            let vb = self.mri_get_vox_val(v.b(), x, y, z, f, 0)?;
                            let mut o = if add {
                                sse::add32(va, vb)
                            } else {
                                sse::sub32(va, vb)
                            };
                            if add && kd == types::UCHAR {
                                o = clip(o, 0.0, 255.0);
                            }
                            self.mri_set_vox_val(v.d_mut(), x, y, z, f, o, 0)?;
                        }
                    }
                }
            }
            return Ok(v.out());
        }
        // one type: direct
        let direct_src = elem_size(ka).is_some();
        let direct_dst = elem_size(kd).is_some();
        // the previous result (0 at first), kept where the source type has no direct access
        let mut last_f = 0.0f32;
        let mut last_i = 0i64;
        let mut last_is_float = true;
        for f in 0..i64::from(nf) {
            for z in 0..i64::from(g.depth) {
                for y in 0..i64::from(g.height) {
                    for x in 0..i64::from(g.width) {
                        if direct_src {
                            let ea = self.direct_get(v.a(), ka, x, y, z, f, what)?;
                            let eb = self.direct_get(v.b(), kb, x, y, z, f, what)?;
                            if !add {
                                last_f = sse::sub32(ea.to_f32(), eb.to_f32());
                                last_is_float = true;
                            } else if ka == types::FLOAT || kd == types::FLOAT {
                                last_f = sse::add32(ea.to_f32(), eb.to_f32());
                                last_is_float = true;
                            } else {
                                let s = match ka {
                                    types::INT if kd != types::LONG => i64::from(
                                        (ea.to_i64() as i32).wrapping_add(eb.to_i64() as i32),
                                    ),
                                    types::LONG => ea.to_i64().wrapping_add(eb.to_i64()),
                                    _ => ea.to_i64() + eb.to_i64(),
                                };
                                last_i = s;
                                last_is_float = false;
                            }
                        }
                        // a destination without direct access is not written; nor, for a sum, any
                        // destination of a source without it (observed: the probe's `vtwo`)
                        if !direct_dst || (add && !direct_src) {
                            continue;
                        }
                        let bytes = if kd == types::FLOAT {
                            let o = if last_is_float { last_f } else { last_i as f32 };
                            o.to_le_bytes().to_vec()
                        } else {
                            let i = if last_is_float {
                                i64::from(self.nint(sse::widen(last_f)))
                            } else {
                                last_i
                            };
                            int_bytes(kd, i)
                        };
                        self.direct_put(v.d_mut(), x, y, z, f, &bytes, what)?;
                    }
                }
            }
        }
        Ok(v.out())
    }

    /// `MRIdivide(a, b, dst)` (§10; FS-292 for three byte volumes).
    pub fn mri_divide(&mut self, a: &mut Mri, b: &mut Mri, dst: Dst) -> Result<VolOut, Stop> {
        let mut v = Vols::new(a, Some(b), dst);
        if !v.has_dst() {
            let d = self.new_with_header(v.a(), types::FLOAT, 1)?;
            v.set_dst(d);
        }
        let g = v.a().geom.clone();
        let bytes =
            v.a().kind == types::UCHAR && v.b().kind == types::UCHAR && v.d().kind == types::UCHAR;
        if !bytes {
            for z in 0..g.depth {
                for y in 0..g.height {
                    for x in 0..g.width {
                        let va = self.mri_get_vox_val(v.a(), x, y, z, 0, 0)?;
                        let vb = self.mri_get_vox_val(v.b(), x, y, z, 0, 0)?;
                        let o = if vb.abs() < f32::EPSILON {
                            0.0
                        } else {
                            sse::narrow(sse::div64(sse::widen(va), sse::widen(vb)))
                        };
                        self.mri_set_vox_val(v.d_mut(), x, y, z, 0, o, 0)?;
                    }
                }
            }
            return Ok(v.out());
        }
        let reproduce = self.mode().reproduce();
        let what = "MRIdivide";
        for z in 0..i64::from(g.depth) {
            for y in 0..i64::from(g.height) {
                // FS-292: FreeSurfer's places in the first volume and the destination stay
                // after a zero divisor
                let mut pa = 0i64;
                for x in 0..i64::from(g.width) {
                    let xa = if reproduce { pa } else { x };
                    let Elem::U8(ua) = self.direct_get(v.a(), types::UCHAR, xa, y, z, 0, what)?
                    else {
                        unreachable!()
                    };
                    let Elem::U8(ub) = self.direct_get(v.b(), types::UCHAR, x, y, z, 0, what)?
                    else {
                        unreachable!()
                    };
                    let o = match ua.checked_div(ub) {
                        Some(q) => q,
                        None => {
                            if ua == 0 {
                                0
                            } else {
                                255
                            }
                        }
                    };
                    self.direct_put(v.d_mut(), xa, y, z, 0, &[o], what)?;
                    if ub == 0 && x + 1 < i64::from(g.width) {
                        if reproduce {
                            self.note(
                                &FS_292,
                                "after a zero divisor the first volume and the destination stay at the same place",
                            );
                        } else {
                            self.note(
                                &FS_292,
                                "after a zero divisor every volume moves on to the next voxel",
                            );
                        }
                    }
                    if ub != 0 || !reproduce {
                        pa += 1;
                    }
                }
            }
        }
        Ok(v.out())
    }

    /// `MRIscaleAndMultiply(a, s, b, dst)` (§10): frame 0, byte voxels (direct): `nint(va * (vb
    /// / s))` clipped to [0, 255] (FS-291).
    pub fn mri_scale_and_multiply(
        &mut self,
        a: &mut Mri,
        s: f32,
        b: &mut Mri,
        dst: Dst,
    ) -> Result<VolOut, Stop> {
        let mut v = Vols::new(a, Some(b), dst);
        if !v.has_dst() {
            let d = self.new_with_header(v.a(), v.a().kind, 1)?;
            v.set_dst(d);
        }
        let name = "MRIscaleAndMultiply";
        self.bytes_only(name, v.a().kind)?;
        for k in [v.b().kind, v.d().kind] {
            if k != v.a().kind {
                self.bytes_only(name, k)?;
            }
        }
        let g = v.a().geom.clone();
        for z in 0..i64::from(g.depth) {
            for y in 0..i64::from(g.height) {
                for x in 0..i64::from(g.width) {
                    let ea = self.direct_get(v.a(), types::UCHAR, x, y, z, 0, name)?;
                    let eb = self.direct_get(v.b(), types::UCHAR, x, y, z, 0, name)?;
                    let p = sse::mul32(ea.to_f32(), sse::div32(eb.to_f32(), s));
                    let i = self.nint(sse::widen(p)).clamp(0, 255);
                    self.direct_put(v.d_mut(), x, y, z, 0, &[i as u8], name)?;
                }
            }
        }
        Ok(v.out())
    }
}

/// Whether a direct place is inside the volume.
pub fn is_inside(p: Place) -> bool {
    matches!(p, Place::In(_))
}
