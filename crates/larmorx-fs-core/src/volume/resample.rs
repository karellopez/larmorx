// SPDX-License-Identifier: Apache-2.0
//! Resolution, planes, fills, resampling by whole factors and changes of type (spec §25):
//! `MRIsetResolution`, `MRIvalueFill`, `MRIfillFG`, `MRIextractPlane`, `MRIfillPlane`, the
//! up- and downsamplers, `MRImeanFrameThresh`, `MRIlimits`, `MRIchangeType` and
//! `MRISeqchangeType`; FS-316 to FS-319.

use larmorx_core::defects::{FS_316, FS_317, FS_318, FS_319};

use super::{Dst, VolOut, Vols};
use crate::clib::printf::sprintf_f64;
use crate::maths::Returned;
use crate::maths::matrix::{MATRIX_REAL, Matrix};
use crate::mri::voxels::Place;
use crate::mri::{Mri, types};
use crate::process::{Process, Stop, codes};
use crate::sse;

/// The plane orientations of `MRIextractPlane` and `MRIfillPlane` (§25).
pub mod orientation {
    pub const CORONAL: i32 = 0;
    pub const SAGITTAL: i32 = 1;
    pub const HORIZONTAL: i32 = 2;
}

/// C's `MAX(a, b)`, `a >= b ? a : b`: a NaN second operand wins, a NaN first operand loses.
fn cmax(a: f32, b: f32) -> f32 {
    if a >= b { a } else { b }
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

/// Whether `MRIchangeType` converts `from` to `to` by the voxel access alone (§25).
fn plain_change(from: i32, to: i32) -> bool {
    match from {
        types::UCHAR => matches!(to, types::SHORT | types::INT | types::LONG | types::FLOAT),
        types::SHORT | types::USHRT => matches!(to, types::INT | types::LONG | types::FLOAT),
        types::LONG => matches!(to, types::INT | types::FLOAT),
        types::INT => matches!(to, types::LONG | types::FLOAT),
        _ => false,
    }
}

/// Whether a range fits the type for `MRIchangeType`'s `no_scale == 1` (§25): bytes and shorts
/// against their ranges in `double`, ints and longs against their limits as `float`s; other
/// types never fit.
fn range_fits(to: i32, min: f32, max: f32) -> bool {
    let (lo, hi) = (f64::from(min), f64::from(max));
    match to {
        types::UCHAR => !(lo < 0.0 || hi > 255.0),
        types::SHORT => !(lo < -32768.0 || hi > 32767.0),
        types::INT => !(min < i32::MIN as f32 || max > i32::MAX as f32),
        types::LONG => !(min < i64::MIN as f32 || max > i64::MAX as f32),
        _ => false,
    }
}

/// The destination range of `MRIchangeType`'s rescaling (§25).
fn dest_range(to: i32) -> (f32, f32) {
    match to {
        types::UCHAR => (0.0, 255.0),
        types::SHORT => (-32768.0, 32767.0),
        types::USHRT => (0.0, 65535.0),
        types::INT => (i32::MIN as f32, i32::MAX as f32),
        types::LONG => (i64::MIN as f32, i64::MAX as f32),
        _ => (0.0, 0.0),
    }
}

/// A `printf` number of a message.
fn fmt(spec: &[u8], v: f64) -> String {
    String::from_utf8_lossy(&sprintf_f64(spec, v)).into_owned()
}

/// A volume's extent and frames seen as other ones (`MRISeqchangeType`'s view of a sequence).
fn reshape(m: &mut Mri, depth: i32, nframes: i32) {
    m.geom.depth = depth;
    m.nframes = nframes;
    let z = |v: i32| v as i64 as u64;
    m.vox_per_vol = m.vox_per_slice.wrapping_mul(z(depth));
    m.vox_total = m.vox_per_vol.wrapping_mul(z(nframes));
}

/// The flood fill's bytes (§25): frame 0's first byte of each voxel position, its row read as
/// bytes through §6's direct access; a byte written where the destination holds no voxel (outside
/// its buffer, or at another voxel by default) is not stored but read back at the same
/// coordinates for the rest of the fill.
#[derive(Default)]
struct ByteWalk {
    unstored: std::collections::HashMap<[i32; 3], u8>,
}

impl ByteWalk {
    fn read(&self, p: &mut Process, m: &Mri, q: [i32; 3]) -> u8 {
        if !self.unstored.is_empty()
            && let Some(b) = self.unstored.get(&q)
        {
            return *b;
        }
        let place = p.row_place(m, i64::from(q[0]), i64::from(q[1]), i64::from(q[2]), 0, 1);
        if let (Place::In(o), Some(v)) = (place, m.voxels.as_deref()) {
            return v[o as usize];
        }
        p.direct_read(m, place, 1, "MRIfillFG")[0]
    }

    fn write(&mut self, p: &mut Process, m: &mut Mri, q: [i32; 3], b: u8) {
        let place = p.row_place(m, i64::from(q[0]), i64::from(q[1]), i64::from(q[2]), 0, 1);
        if let (Place::In(o), Some(v)) = (place, m.voxels.as_deref_mut()) {
            v[o as usize] = b;
            if !self.unstored.is_empty() {
                self.unstored.remove(&q);
            }
            return;
        }
        let stored = matches!(place, Place::Other(_)) && p.mode().reproduce();
        p.direct_write(m, place, &[b], "MRIfillFG");
        if stored {
            self.unstored.remove(&q);
        } else {
            self.unstored.insert(q, b);
        }
    }
}

/// The offsets of frame 0's first byte of each voxel position of a `w x h x d` walk in `m`
/// (§25's byte walk), when every one lies inside the volume (no tensor rows, FS-960).
fn byte_offsets(m: &Mri, w: i32, h: i32, d: i32) -> Option<(usize, usize)> {
    let g = &m.geom;
    let row = i64::from(g.width) * m.bytes_per_vox as i64;
    let inside = m.kind != types::TENSOR
        && m.nframes >= 1
        && h <= g.height
        && d <= g.depth
        && i64::from(w) <= row;
    let len = m.voxels.as_ref()?.len() as i64;
    let need = (i64::from(g.depth) * i64::from(g.height)) * row;
    (inside && need <= len).then_some((row as usize, g.height as usize))
}

/// `MRIfillFG`'s fill when every byte it reads or writes lies inside both volumes (§25): the
/// sweeps then fill exactly the voxels joined to the seed by face neighbours whose source byte is
/// at least `threshold`, so a walk of that set gives the same bytes and count. `None` where a byte
/// lies elsewhere (the sweeps' own walk then follows §6's places).
fn fill_fast(v: &mut Vols, e: [i32; 3], s: [i32; 3], threshold: i32, fill_val: i32) -> Option<i32> {
    let (w, h, d) = (e[0], e[1], e[2]);
    if (0..3).any(|k| s[k] < 0 || s[k] >= e[k]) {
        return None;
    }
    let (srow, sh) = byte_offsets(v.a(), w, h, d)?;
    let (drow, dh) = byte_offsets(v.d(), w, h, d)?;
    let same = v.d_is_a();
    let (wu, hu, du) = (w as usize, h as usize, d as usize);
    let n = wu * hu * du;
    let at = |row: usize, height: usize, x: usize, y: usize, z: usize| (z * height + y) * row + x;
    let mut dst = vec![0u8; n];
    {
        let dv = v.d().voxels.as_deref()?;
        for z in 0..du {
            for y in 0..hu {
                let o = at(drow, dh, 0, y, z);
                dst[(z * hu + y) * wu..(z * hu + y + 1) * wu].copy_from_slice(&dv[o..o + wu]);
            }
        }
    }
    let src = if same {
        None
    } else {
        let sv = v.a().voxels.as_deref()?;
        let mut b = vec![0u8; n];
        for z in 0..du {
            for y in 0..hu {
                let o = at(srow, sh, 0, y, z);
                b[(z * hu + y) * wu..(z * hu + y + 1) * wu].copy_from_slice(&sv[o..o + wu]);
            }
        }
        Some(b)
    };
    let ok = (0..=255).contains(&fill_val);
    let fv = fill_val as u8;
    if ok {
        let low = fill_val.wrapping_sub(1) as u8;
        for b in dst.iter_mut() {
            if *b == fv {
                *b = low;
            }
        }
    }
    let seed = (s[2] as usize * hu + s[1] as usize) * wu + s[0] as usize;
    dst[seed] = fv;
    let mut filled = 0i32;
    if ok {
        let mut stack = vec![seed];
        while let Some(i) = stack.pop() {
            let (x, y, z) = (i % wu, (i / wu) % hu, i / (wu * hu));
            let mut visit = |j: usize, dst: &mut Vec<u8>, stack: &mut Vec<usize>| {
                let sb = src.as_ref().map_or(dst[j], |b| b[j]);
                if dst[j] != fv && i32::from(sb) >= threshold {
                    dst[j] = fv;
                    filled += 1;
                    stack.push(j);
                }
            };
            if x > 0 {
                visit(i - 1, &mut dst, &mut stack);
            }
            if x + 1 < wu {
                visit(i + 1, &mut dst, &mut stack);
            }
            if y > 0 {
                visit(i - wu, &mut dst, &mut stack);
            }
            if y + 1 < hu {
                visit(i + wu, &mut dst, &mut stack);
            }
            if z > 0 {
                visit(i - wu * hu, &mut dst, &mut stack);
            }
            if z + 1 < du {
                visit(i + wu * hu, &mut dst, &mut stack);
            }
        }
    }
    let dv = v.d_mut().voxels.as_deref_mut()?;
    for z in 0..du {
        for y in 0..hu {
            let o = at(drow, dh, 0, y, z);
            dv[o..o + wu].copy_from_slice(&dst[(z * hu + y) * wu..(z * hu + y + 1) * wu]);
        }
    }
    Some(filled)
}

impl Process {
    /// FS-317: C's conversion of a number to an `int` (x86-64's truncation). A NaN or a number
    /// beyond the `int` range gives -2147483648 with `reproduce`; by default NaN gives 0 and a
    /// number beyond the range the range's nearer end; noted in both modes.
    pub(crate) fn int_317(&mut self, v: f64, what: &str) -> i32 {
        if v > -2_147_483_649.0 && v < 2_147_483_648.0 {
            return v.trunc() as i32;
        }
        if self.mode().reproduce() {
            self.note(
                &FS_317,
                format!("{what}: a number that is NaN or beyond the int range converted to -2147483648, as FreeSurfer's"),
            );
            return i32::MIN;
        }
        self.note(
            &FS_317,
            format!("{what}: a number that is NaN or beyond the int range converted to 0 (NaN) or the range's nearer end (FreeSurfer gives -2147483648)"),
        );
        if v.is_nan() {
            0
        } else if v > 0.0 {
            i32::MAX
        } else {
            i32::MIN
        }
    }

    /// The source's voxel-to-RAS matrix applied to `(c, r, s, 1)` (§25's centres): a float
    /// matrix product.
    fn vox2ras_point(&mut self, m: &Mri, p: [f32; 3]) -> Result<[f32; 3], Stop> {
        let a = self.extract_i_to_r(&m.geom)?;
        let mut v = Matrix::zeros(4, 1, MATRIX_REAL);
        for (k, x) in p.iter().enumerate() {
            v.set(k as i32 + 1, 1, *x);
        }
        v.set(4, 1, 1.0);
        match self.matrix_multiply(Some(&a), Some(&v), None)? {
            Returned::New(c) => Ok([c.at(1, 1), c.at(2, 1), c.at(3, 1)]),
            _ => Ok([0.0; 3]),
        }
    }

    /// `MRIsetResolution(mri, xres, yres, zres)` (§25): the voxel sizes, `ps`, the starts and
    /// ends scaled by their axis, `thick` the largest; the cached matrices recomputed. Returns 0.
    pub fn mri_set_resolution(
        &mut self,
        m: &mut Mri,
        xres: f32,
        yres: f32,
        zres: f32,
    ) -> Result<i32, Stop> {
        m.geom.xsize = xres;
        m.geom.ysize = yres;
        m.geom.zsize = zres;
        m.ps = sse::div32(sse::add32(sse::add32(xres, yres), zres), 3.0);
        m.xstart = sse::mul32(xres, m.xstart);
        m.xend = sse::mul32(xres, m.xend);
        m.ystart = sse::mul32(yres, m.ystart);
        m.yend = sse::mul32(m.yend, yres);
        m.zstart = sse::mul32(zres, m.zstart);
        m.zend = sse::mul32(m.zend, zres);
        m.thick = cmax(cmax(xres, yres), zres);
        self.reinit_cache(&mut m.geom)?;
        Ok(0)
    }

    /// `MRIvalueFill(mri, value)` (§25): every voxel of every frame `value` (the integer types
    /// `nint(value)`'s low bits, floats the float's bits); other types unchanged; FS-58 without
    /// voxels. Returns 0.
    pub fn mri_value_fill(&mut self, m: &mut Mri, value: f32) -> Result<i32, Stop> {
        let kind = m.kind;
        if !matches!(
            kind,
            types::UCHAR | types::SHORT | types::USHRT | types::INT | types::FLOAT
        ) {
            return Ok(0);
        }
        let g = &m.geom;
        let z = |v: i32| v.max(0) as usize;
        let count = z(g.width) * z(g.height) * z(g.depth) * z(m.nframes);
        if count == 0 {
            return Ok(0);
        }
        if !m.has_voxels() {
            self.no_voxels("MRIvalueFill")?;
            return Ok(0);
        }
        let bytes = if kind == types::FLOAT {
            value.to_le_bytes().to_vec()
        } else {
            let n = self.nint(sse::widen(value));
            int_bytes(kind, i64::from(n))
        };
        let n = bytes.len();
        if let Some(v) = m.voxels.as_deref_mut() {
            let end = (count * n).min(v.len());
            for c in v[..end].chunks_exact_mut(n) {
                c.copy_from_slice(&bytes);
            }
        }
        Ok(0)
    }

    /// `MRIfillFG(src, dst, seed_x, seed_y, seed_z, threshold, fill_val, &npix)` (§25): a flood
    /// fill on frame 0's first byte of each voxel position, from the clamped seed (FS-316), into
    /// `dst` (a clone of `src` without one); `npix` 1 plus the voxels filled.
    #[allow(clippy::too_many_arguments)]
    pub fn mri_fill_fg(
        &mut self,
        src: &mut Mri,
        dst: Dst,
        seed: [i32; 3],
        threshold: i32,
        fill_val: i32,
        npix: &mut i32,
    ) -> Result<VolOut, Stop> {
        let e = [src.geom.width, src.geom.height, src.geom.depth];
        let mut v = Vols::new(src, None, dst);
        if !v.has_dst() {
            let c = self.mri_clone_new(v.a())?;
            v.set_dst(c);
        }
        self.direct_needs_voxels(v.a(), "MRIfillFG")?;
        self.direct_needs_voxels(v.d(), "MRIfillFG")?;
        let reproduce = self.mode().reproduce();
        let mut s = seed;
        s[0] = if s[0] < 0 {
            0
        } else if s[0] >= e[0] {
            e[0] - 1
        } else {
            s[0]
        };
        for k in 1..3 {
            if s[k] < 0 {
                s[k] = 0;
            } else if s[k] >= e[k] {
                let axis = if k == 1 { "row" } else { "slice" };
                if reproduce {
                    self.note(
                        &FS_316,
                        format!("MRIfillFG: a seed {axis} at or past the extent clamped to the width minus one, as FreeSurfer's"),
                    );
                    s[k] = e[0] - 1;
                } else {
                    self.note(
                        &FS_316,
                        format!("MRIfillFG: a seed {axis} at or past the extent clamped to the extent minus one (FreeSurfer clamps it to the width minus one)"),
                    );
                    s[k] = e[k] - 1;
                }
            }
        }
        if let Some(filled) = fill_fast(&mut v, e, s, threshold, fill_val) {
            *npix = filled.wrapping_add(1);
            return Ok(v.out());
        }
        let fv = fill_val as u8;
        let same = v.d_is_a();
        let mut dw = ByteWalk::default();
        let none = ByteWalk::default();
        let matches = |b: u8| i32::from(b) == fill_val;
        // every byte equal to fill_val becomes fill_val - 1
        for z in 0..e[2] {
            for y in 0..e[1] {
                for x in 0..e[0] {
                    let b = dw.read(self, v.d(), [x, y, z]);
                    if matches(b) {
                        dw.write(self, v.d_mut(), [x, y, z], fill_val.wrapping_sub(1) as u8);
                    }
                }
            }
        }
        dw.write(self, v.d_mut(), s, fv);
        let clampb = |c: i32, k: usize| c.max(0).min(e[k] - 1);
        let mut lo = [0i32; 3];
        let mut hi = [0i32; 3];
        for k in 0..3 {
            lo[k] = clampb(s[k] - 1, k);
            hi[k] = clampb(s[k] + 1, k);
        }
        let mut total = 0i32;
        loop {
            let mut filled = 0i32;
            let (mut nlo, mut nhi) = (lo, hi);
            for z in lo[2]..=hi[2] {
                for y in lo[1]..=hi[1] {
                    for x in lo[0]..=hi[0] {
                        let sb = if same { &dw } else { &none }.read(self, v.a(), [x, y, z]);
                        if i32::from(sb) < threshold {
                            continue;
                        }
                        let db = dw.read(self, v.d(), [x, y, z]);
                        if matches(db) {
                            continue;
                        }
                        let p = [x, y, z];
                        let mut next = false;
                        for k in 0..3 {
                            for step in [-1, 1] {
                                let mut q = p;
                                q[k] += step;
                                if q[k] < 0 || q[k] >= e[k] {
                                    continue;
                                }
                                if matches(dw.read(self, v.d(), q)) {
                                    next = true;
                                }
                            }
                        }
                        if !next {
                            continue;
                        }
                        dw.write(self, v.d_mut(), [x, y, z], fv);
                        filled += 1;
                        for k in 0..3 {
                            if p[k] == lo[k] {
                                nlo[k] = clampb(p[k] - 1, k);
                            }
                            if p[k] == hi[k] {
                                nhi[k] = clampb(p[k] + 1, k);
                            }
                        }
                    }
                }
            }
            if filled == 0 {
                break;
            }
            total = total.wrapping_add(filled);
            lo = nlo;
            hi = nhi;
        }
        *npix = total.wrapping_add(1);
        Ok(v.out())
    }

    /// `MRIextractPlane(src, dst, orientation, where)` (§25): a slice (coronal and unknown
    /// orientations), row (horizontal) or column (sagittal) of frame 0 as a one-slice volume.
    pub fn mri_extract_plane(
        &mut self,
        src: &mut Mri,
        dst: Dst,
        orient: i32,
        at: i32,
    ) -> Result<VolOut, Stop> {
        let (w, h, d) = (src.geom.width, src.geom.height, src.geom.depth);
        let (pw, ph) = match orient {
            orientation::SAGITTAL => (d, h),
            orientation::HORIZONTAL => (w, d),
            _ => (w, h),
        };
        let mut v = Vols::new(src, None, dst);
        if !v.has_dst() {
            let mut n = self.mri_alloc_sequence(pw, ph, 1, v.a().kind, 1)?;
            self.copy_header_into(v.a(), &mut n)?;
            n.zstart = at as f32;
            n.zend = at.wrapping_add(1) as f32;
            n.imnr0 = 0;
            n.imnr1 = 1;
            v.set_dst(n);
        }
        for x in 0..pw {
            for y in 0..ph {
                let (c, r, s) = match orient {
                    orientation::SAGITTAL => (at, y, x),
                    orientation::HORIZONTAL => (x, at, y),
                    _ => (x, y, at),
                };
                let val = self.mri_get_vox_val(v.a(), c, r, s, 0, 0)?;
                self.mri_set_vox_val(v.d_mut(), x, y, 0, 0, val, 0)?;
            }
        }
        Ok(v.out())
    }

    /// `MRIfillPlane(mask, dst, orientation, where, fillval)` (§25): over `dst`'s extent, every
    /// voxel of the mask's slice 0 that is not 0 sets `dst`'s voxel of plane `where` to `fillval`
    /// (an unknown orientation is coronal).
    pub fn mri_fill_plane(
        &mut self,
        mask: &Mri,
        dst: &mut Mri,
        orient: i32,
        at: i32,
        fillval: i32,
    ) -> Result<(), Stop> {
        let (w, h, d) = (dst.geom.width, dst.geom.height, dst.geom.depth);
        let fv = fillval as f32;
        match orient {
            orientation::HORIZONTAL => {
                for z in 0..d {
                    for x in 0..w {
                        if self.mri_get_vox_val(mask, x, z, 0, 0, 0)? != 0.0 {
                            self.mri_set_vox_val(dst, x, at, z, 0, fv, 0)?;
                        }
                    }
                }
            }
            orientation::SAGITTAL => {
                for z in 0..d {
                    for y in 0..h {
                        if self.mri_get_vox_val(mask, z, y, 0, 0, 0)? != 0.0 {
                            self.mri_set_vox_val(dst, at, y, z, 0, fv, 0)?;
                        }
                    }
                }
            }
            // coronal, and any unknown orientation (observed)
            _ => {
                for y in 0..h {
                    for x in 0..w {
                        if self.mri_get_vox_val(mask, x, y, 0, 0, 0)? != 0.0 {
                            self.mri_set_vox_val(dst, x, y, at, 0, fv, 0)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// `MRIupsampleN(src, dst, N)` (§25): N times the source's extent, each voxel the source's
    /// at the integer quotients; the voxel sizes divided by N and the first voxel placed at the
    /// source's `(c, c, c)`, `c = -0.5 + 1 / (2N)`.
    pub fn mri_upsample_n(&mut self, src: &mut Mri, dst: Dst, n: i32) -> Result<VolOut, Stop> {
        let mut v = Vols::new(src, None, dst);
        self.upsample_into(&mut v, n)?;
        Ok(v.out())
    }

    /// `MRIupsample2(src, dst)` (§25): `MRIupsampleN` by 2.
    pub fn mri_upsample2(&mut self, src: &mut Mri, dst: Dst) -> Result<VolOut, Stop> {
        self.mri_upsample_n(src, dst, 2)
    }

    fn upsample_into(&mut self, v: &mut Vols, n: i32) -> Result<(), Stop> {
        let (w, h, d) = (v.a().geom.width, v.a().geom.height, v.a().geom.depth);
        let (nw, nh, nd) = (w.wrapping_mul(n), h.wrapping_mul(n), d.wrapping_mul(n));
        let nframes = v.a().nframes;
        if !v.has_dst() {
            let kind = v.a().kind;
            let mut m = self.mri_alloc_sequence(nw, nh, nd, kind, nframes)?;
            self.copy_header_into(v.a(), &mut m)?;
            v.set_dst(m);
        }
        let nf = n as f32;
        let a = v.a().clone();
        {
            let dm = v.d_mut();
            dm.geom.xsize = sse::div32(a.geom.xsize, nf);
            dm.geom.ysize = sse::div32(a.geom.ysize, nf);
            dm.geom.zsize = sse::div32(a.geom.zsize, nf);
            dm.geom.x = a.geom.x;
            dm.geom.y = a.geom.y;
            dm.geom.z = a.geom.z;
            dm.xstart = a.xstart;
            dm.ystart = a.ystart;
            dm.zstart = a.zstart;
            dm.xend = a.xend;
            dm.yend = a.yend;
            dm.zend = a.zend;
            dm.imnr0 = a.imnr0;
            dm.imnr1 = dm.imnr0.wrapping_add(dm.geom.depth).wrapping_sub(1);
        }
        let c = sse::narrow(sse::add64(
            -0.5,
            sse::div64(1.0, 2.0 * f64::from(n)),
        ));
        let p = self.vox2ras_point(v.a(), [c, c, c])?;
        self.p0_to_cras(
            &mut v.d_mut().geom,
            sse::widen(p[0]),
            sse::widen(p[1]),
            sse::widen(p[2]),
        )?;
        for f in 0..nframes {
            for z in 0..nd {
                for y in 0..nh {
                    for x in 0..nw {
                        let val = self.mri_get_vox_val(v.a(), x / n, y / n, z / n, f, 0)?;
                        self.mri_set_vox_val(v.d_mut(), x, y, z, f, val, 0)?;
                    }
                }
            }
        }
        self.reinit_cache(&mut v.d_mut().geom)?;
        Ok(())
    }

    /// `MRIupsampleNConserve(src, dst, N)` (§25): a source that is not float changed to floats
    /// (`MRISeqchangeType`), upsampled, every voxel divided by `N * N * N`. FS-319: a change of
    /// type that fails ends the program (FreeSurfer crashes).
    pub fn mri_upsample_n_conserve(
        &mut self,
        src: &mut Mri,
        dst: Dst,
        n: i32,
    ) -> Result<VolOut, Stop> {
        let out = if src.kind == types::FLOAT {
            let mut v = Vols::new(src, None, dst);
            self.upsample_into(&mut v, n)?;
            self.conserve_divide(v.d_mut(), n)?;
            v.out()
        } else {
            let Some(mut t) = self.mri_seq_change_type(src, types::FLOAT, 0.0, 0.0, 0)? else {
                self.note(
                    &FS_319,
                    "MRIupsampleNConserve: no volume after a failed change to floats: the program ends (FreeSurfer crashes)",
                );
                self.error_exit(
                    codes::BAD_PARM,
                    b"MRIupsampleNConserve(): no volume to upsample (FreeSurfer crashes here)\n",
                )?;
                return Ok(VolOut::Nothing);
            };
            match dst {
                Dst::Src | Dst::Other => {
                    let mut v = Vols::new(&mut t, None, Dst::Given(src));
                    self.upsample_into(&mut v, n)?;
                    self.conserve_divide(v.d_mut(), n)?;
                    VolOut::Src
                }
                d => {
                    let mut v = Vols::new(&mut t, None, d);
                    self.upsample_into(&mut v, n)?;
                    self.conserve_divide(v.d_mut(), n)?;
                    match v.out() {
                        VolOut::Src => VolOut::Nothing,
                        o => o,
                    }
                }
            }
        };
        Ok(out)
    }

    /// The division of every voxel of `MRIupsampleNConserve`'s result by `N * N * N` (a 32-bit
    /// product, in `double`).
    pub(crate) fn conserve_divide(&mut self, m: &mut Mri, n: i32) -> Result<(), Stop> {
        let div = f64::from(n.wrapping_mul(n).wrapping_mul(n));
        let (w, h, d, nf) = (m.geom.width, m.geom.height, m.geom.depth, m.nframes);
        for f in 0..nf {
            for z in 0..d {
                for y in 0..h {
                    for x in 0..w {
                        let val = self.mri_get_vox_val(m, x, y, z, f, 0)?;
                        let q = sse::narrow(sse::div64(sse::widen(val), div));
                        self.mri_set_vox_val(m, x, y, z, f, q, 0)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// `MRIdownsample2(src, dst)` (§25): each 2 x 2 x 2 block of frame 0 averaged (bytes,
    /// shorts, unsigned shorts and floats); the voxel sizes, `thick` and `ps` doubled.
    pub fn mri_downsample2(&mut self, src: &mut Mri, dst: Dst) -> Result<VolOut, Stop> {
        if let Dst::Given(g) = &dst
            && g.kind != src.kind
        {
            self.error_printf(
                codes::UNSUPPORTED,
                b"MRIdownsample2: source and dst must be same type",
            );
            return Ok(VolOut::Nothing);
        }
        let (w, h, d) = (src.geom.width, src.geom.height, src.geom.depth);
        let (nw, nh, nd) = (w / 2, h / 2, d / 2);
        let kind = src.kind;
        let mut v = Vols::new(src, None, dst);
        if !v.has_dst() {
            let mut m = self.mri_alloc_sequence(nw, nh, nd, kind, 1)?;
            self.copy_header_into(v.a(), &mut m)?;
            v.set_dst(m);
        }
        self.mri_clear(v.d_mut())?;
        for z in 0..nd {
            for y in 0..nh {
                for x in 0..nw {
                    let mut sum = 0.0f32;
                    if !matches!(
                        kind,
                        types::UCHAR | types::SHORT | types::USHRT | types::FLOAT
                    ) {
                        self.error_printf(
                            codes::UNSUPPORTED,
                            format!("MRIdownsample2: unsupported input type {kind}").as_bytes(),
                        );
                        return Ok(VolOut::Nothing);
                    }
                    for dz in 0..2 {
                        for dy in 0..2 {
                            for dx in 0..2 {
                                let val = self.mri_get_vox_val(
                                    v.a(),
                                    2 * x + dx,
                                    2 * y + dy,
                                    2 * z + dz,
                                    0,
                                    0,
                                )?;
                                sum = sse::add32(sum, val);
                            }
                        }
                    }
                    let val = sse::mul32(sum, 0.125);
                    let b = if kind == types::FLOAT {
                        val.to_le_bytes().to_vec()
                    } else {
                        let r = self.nint(sse::widen(val));
                        int_bytes(kind, i64::from(r))
                    };
                    self.direct_put(
                        v.d_mut(),
                        i64::from(x),
                        i64::from(y),
                        i64::from(z),
                        0,
                        &b,
                        "MRIdownsample2",
                    )?;
                }
            }
        }
        let a = v.a().clone();
        {
            let dm = v.d_mut();
            dm.imnr0 = a.imnr0;
            dm.imnr1 = dm.imnr0.wrapping_add(dm.geom.depth).wrapping_sub(1);
            dm.geom.xsize = sse::mul32(a.geom.xsize, 2.0);
            dm.geom.ysize = sse::mul32(a.geom.ysize, 2.0);
            dm.geom.zsize = sse::mul32(a.geom.zsize, 2.0);
            dm.thick = sse::mul32(a.thick, 2.0);
            dm.ps = sse::mul32(a.ps, 2.0);
        }
        // the source's vox2ras after the sizes changed (the destination may be the source)
        let half = |e: i32| (e / 2) as f32 + 0.5;
        let c = self.vox2ras_point(v.a(), [half(w), half(h), half(d)])?;
        let dm = v.d_mut();
        dm.geom.c = c;
        self.reinit_cache(&mut dm.geom)?;
        Ok(v.out())
    }

    /// `MRIdownsampleN(src, dst, Fc, Fr, Fs, KeepType)` (§25): each voxel the mean of the
    /// source's voxels in its window (FS-317 for the extents and windows).
    #[allow(clippy::too_many_arguments)]
    pub fn mri_downsample_n(
        &mut self,
        src: &mut Mri,
        dst: Dst,
        fc: f32,
        fr: f32,
        fs: f32,
        keep_type: i32,
    ) -> Result<VolOut, Stop> {
        let (w, h, d, nframes) = (src.geom.width, src.geom.height, src.geom.depth, src.nframes);
        let ext = |p: &mut Process, e: i32, f: f32| {
            p.int_317(
                f64::from(sse::div32(e as f32, f).ceil()),
                "MRIdownsampleN",
            )
        };
        let (nc, nr, ns) = (ext(self, w, fc), ext(self, h, fr), ext(self, d, fs));
        let mut v = Vols::new(src, None, dst);
        if v.has_dst() {
            let g = &v.d().geom;
            if g.width != nc || g.height != nr || g.depth != ns || v.d().nframes != nframes {
                self.put_out(b"ERROR: MRIdownsampleN: dimension mismatch\n");
                return Ok(VolOut::Nothing);
            }
        } else {
            let kind = if keep_type != 0 { v.a().kind } else { types::FLOAT };
            let mut m = self.mri_alloc_sequence(nc, nr, ns, kind, nframes)?;
            self.copy_header_into(v.a(), &mut m)?;
            let a = v.a();
            m.imnr0 = a.imnr0;
            m.imnr1 = m.imnr0.wrapping_add(m.geom.depth).wrapping_sub(1);
            m.geom.xsize = sse::mul32(a.geom.xsize, fc);
            m.geom.ysize = sse::mul32(fr, a.geom.ysize);
            m.geom.zsize = sse::mul32(a.geom.zsize, fs);
            m.thick = sse::mul32(a.thick, fs);
            m.ps = sse::mul32(a.ps, fc);
            let half = |f: f32| sse::narrow(sse::sub64(sse::widen(sse::div32(f, 2.0)), 0.5));
            let p = self.vox2ras_point(v.a(), [half(fc), half(fr), half(fs)])?;
            self.p0_to_cras(
                &mut m.geom,
                sse::widen(p[0]),
                sse::widen(p[1]),
                sse::widen(p[2]),
            )?;
            self.reinit_cache(&mut m.geom)?;
            v.set_dst(m);
        }
        let div = sse::widen(sse::mul32(sse::mul32(fc, fr), fs));
        for f in 0..nframes {
            for c in 0..nc {
                for r in 0..nr {
                    for s in 0..ns {
                        let c1 = sse::mul32((c + 1) as f32, fc);
                        let r1 = sse::mul32((r + 1) as f32, fr);
                        let s1 = sse::mul32((s + 1) as f32, fs);
                        let mut sum = 0.0f64;
                        let mut c0 = self.int_317(
                            sse::widen(sse::mul32(c as f32, fc)),
                            "MRIdownsampleN",
                        );
                        while (c0 as f32) < c1 {
                            let mut r0 = self.int_317(
                                sse::widen(sse::mul32(r as f32, fr)),
                                "MRIdownsampleN",
                            );
                            while (r0 as f32) < r1 {
                                let mut s0 = self.int_317(
                                    sse::widen(sse::mul32(s as f32, fs)),
                                    "MRIdownsampleN",
                                );
                                while (s0 as f32) < s1 {
                                    let val = self.mri_get_vox_val(v.a(), c0, r0, s0, f, 0)?;
                                    sum = sse::add64(sse::widen(val), sum);
                                    s0 = s0.wrapping_add(1);
                                }
                                r0 = r0.wrapping_add(1);
                            }
                            c0 = c0.wrapping_add(1);
                        }
                        let q = sse::narrow(sse::div64(sum, div));
                        self.mri_set_vox_val(v.d_mut(), c, r, s, f, q, 0)?;
                    }
                }
            }
        }
        Ok(v.out())
    }

    /// `MRImeanFrameThresh(mri, frame, thresh)` (§25): the mean of the nearest samples of a frame
    /// that are not below `thresh` (FS-318: FreeSurfer's of frame 0).
    pub fn mri_mean_frame_thresh(
        &mut self,
        m: &Mri,
        frame: i32,
        thresh: f32,
    ) -> Result<f64, Stop> {
        if frame >= m.nframes {
            self.error_printf(
                codes::BAD_PARM,
                format!("MRImeanFrame: frame {frame} out of bounds ({})", m.nframes).as_bytes(),
            );
            return Ok(0.0);
        }
        let f = if frame == 0 {
            0
        } else if self.mode().reproduce() {
            self.note(
                &FS_318,
                "MRImeanFrameThresh: the mean of frame 0's voxels, as FreeSurfer's",
            );
            0
        } else {
            self.note(
                &FS_318,
                "MRImeanFrameThresh: the mean of the frame asked for (FreeSurfer's is frame 0's)",
            );
            frame
        };
        let t = sse::widen(thresh);
        let (mut sum, mut n) = (0.0f64, 0i32);
        for z in 0..m.geom.depth {
            for y in 0..m.geom.height {
                for x in 0..m.geom.width {
                    let mut val = 0.0;
                    self.nearest_frame(
                        m,
                        f64::from(x),
                        f64::from(y),
                        f64::from(z),
                        f,
                        "MRIsampleVolumeType",
                        &mut val,
                    )?;
                    if !(val < t) {
                        sum = sse::add64(val, sum);
                        n = n.wrapping_add(1);
                    }
                }
            }
        }
        Ok(sse::div64(sum, f64::from(n)))
    }

    /// `MRIlimits(mri, &min, &max)` (§25): the smallest and largest voxel; a volume without
    /// voxels leaves both. Returns 0.
    pub fn mri_limits(&mut self, m: &Mri, min: &mut f32, max: &mut f32) -> Result<i32, Stop> {
        if !m.has_voxels() {
            return Ok(0);
        }
        let mut lo = self.mri_get_vox_val(m, 0, 0, 0, 0, 0)?;
        let mut hi = lo;
        for f in 0..m.nframes {
            for z in 0..m.geom.depth {
                for y in 0..m.geom.height {
                    for x in 0..m.geom.width {
                        let val = self.mri_get_vox_val(m, x, y, z, f, 0)?;
                        if val < lo {
                            lo = val;
                        }
                        if val > hi {
                            hi = val;
                        }
                    }
                }
            }
        }
        *min = lo;
        *max = hi;
        Ok(0)
    }

    /// The values of `src` into `dst` as the voxel access converts them (every frame).
    fn copy_values(&mut self, src: &Mri, dst: &mut Mri) -> Result<(), Stop> {
        for f in 0..src.nframes {
            for z in 0..src.geom.depth {
                for y in 0..src.geom.height {
                    for x in 0..src.geom.width {
                        let val = self.mri_get_vox_val(src, x, y, z, f, 0)?;
                        self.mri_set_vox_val(dst, x, y, z, f, val, 0)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// A new volume of the source's extent and frames with `kind` and its header.
    fn change_type_dst(&mut self, src: &Mri, kind: i32) -> Result<Mri, Stop> {
        let g = &src.geom;
        let mut d = self.mri_alloc_sequence(g.width, g.height, g.depth, kind, src.nframes)?;
        self.copy_header_into(src, &mut d)?;
        Ok(d)
    }

    /// `MRIchangeType(src, type, f_low, f_high, no_scale)` (§25): a copy of the same type, the
    /// values converted, or rescaled from the robust range of the histogram (FS-317 for the
    /// bins and counts); `None` after the error when the range is empty.
    pub fn mri_change_type(
        &mut self,
        src: &Mri,
        to: i32,
        f_low: f32,
        f_high: f32,
        no_scale: i32,
    ) -> Result<Option<Mri>, Stop> {
        if src.kind == to {
            return Ok(match self.mri_copy(src, None)? {
                Returned::New(m) => Some(m),
                _ => None,
            });
        }
        if plain_change(src.kind, to) {
            let mut d = self.change_type_dst(src, to)?;
            self.copy_values(src, &mut d)?;
            return Ok(Some(d));
        }
        let (mut min, mut max) = (0.0f32, 0.0f32);
        self.mri_limits(src, &mut min, &mut max)?;
        if no_scale > 1 || (no_scale == 1 && range_fits(to, min, max)) {
            let mut d = self.change_type_dst(src, to)?;
            self.copy_values(src, &mut d)?;
            return Ok(Some(d));
        }
        if no_scale == 1 {
            let line = format!(
                "******* WARNING - forcing scaling of values to prevent cropping of input [{}, {}]\n",
                fmt(b"%2.0f", f64::from(min)),
                fmt(b"%2.0f", f64::from(max))
            );
            self.put_out(line.as_bytes());
            self.put_out(b"***** DISABLING forcing and not doing scaling ********\n");
            let mut d = self.change_type_dst(src, to)?;
            self.copy_values(src, &mut d)?;
            return Ok(Some(d));
        }
        let line = format!(
            "MRIchangeType: Building histogram {} {} 1000, flo={}, fhi={}, dest_type={to}\n",
            fmt(b"%g", f64::from(min)),
            fmt(b"%g", f64::from(max)),
            fmt(b"%g", f64::from(f_low)),
            fmt(b"%g", f64::from(f_high))
        );
        self.put_out(line.as_bytes());
        let mut size = sse::div32(sse::sub32(max, min), 1000.0);
        if self.env.is_set("FS_FORCE_BIN_CHECK") {
            let mean = self.mri_mean_frame_thresh(src, 0, 1e-7)?;
            let off = sse::sub64(mean, sse::widen(min));
            let bin = self.int_317(sse::div64(off, sse::widen(size)), "MRIchangeType");
            if (bin as f32) < 200.0 {
                let new = sse::narrow(sse::div64(off, 200.0));
                let line = format!(
                    "MRIchangeType: original bin size {} (max {}) too big for mean/min {}/{}, scaling down to {}\n",
                    fmt(b"%2.2f", sse::widen(size)),
                    fmt(b"%2.1f", sse::widen(max)),
                    fmt(b"%2.2f", mean),
                    fmt(b"%2.2f", sse::widen(min)),
                    fmt(b"%2.2f", sse::widen(new))
                );
                self.put_out(line.as_bytes());
                size = new;
            }
        }
        let mut hist = vec![0i64; 1000];
        let mut nonzero = 0i32;
        let (w, h, d) = (src.geom.width, src.geom.height, src.geom.depth);
        for f in 0..src.nframes {
            for x in 0..w {
                for y in 0..h {
                    for z in 0..d {
                        let val = self.mri_get_vox_val(src, x, y, z, f, 0)?;
                        if !(f64::from(val.abs()) < 1e-15) {
                            nonzero = nonzero.wrapping_add(1);
                        }
                        let q = sse::div32(sse::sub32(val, min), size);
                        let bin = self.int_317(sse::widen(q), "MRIchangeType").clamp(0, 999);
                        hist[bin as usize] += 1;
                    }
                }
            }
        }
        let target = self.int_317(
            sse::widen(sse::mul32(
                sse::mul32(sse::mul32(w as f32, f_low), h as f32),
                d as f32,
            )),
            "MRIchangeType",
        );
        let (mut bin, mut total) = (0i32, 0i64);
        while total < i64::from(target) && bin < 999 {
            total += hist[bin as usize];
            bin += 1;
        }
        min = sse::add32(min, sse::mul32(bin as f32, size));
        let target = self.int_317(
            sse::mul64(f64::from(nonzero), sse::sub64(1.0, sse::widen(f_high))),
            "MRIchangeType",
        );
        let (mut bin, mut total) = (999i32, 0i64);
        while total < i64::from(target) && bin > 0 {
            total += hist[bin as usize];
            bin -= 1;
        }
        max = sse::add32(sse::mul32(bin as f32, size), min);
        if min > max {
            let m = format!(
                "MRIchangeType(): after hist: src_min = {}, src_max = {} (f_low = {}, f_high = {})",
                fmt(b"%g", f64::from(min)),
                fmt(b"%g", f64::from(max)),
                fmt(b"%g", f64::from(f_low)),
                fmt(b"%g", f64::from(f_high))
            );
            self.error_printf(codes::BAD_PARM, m.as_bytes());
            return Ok(None);
        }
        let (dmin, dmax) = dest_range(to);
        let scale = if max == min {
            1.0
        } else {
            sse::div32(sse::sub32(dmax, dmin), sse::sub32(max, min))
        };
        let mut dv = self.change_type_dst(src, to)?;
        for f in 0..src.nframes {
            for z in 0..d {
                for y in 0..h {
                    for x in 0..w {
                        let val = self.mri_get_vox_val(src, x, y, z, f, 0)?;
                        let q = sse::add32(sse::mul32(sse::sub32(val, min), scale), dmin);
                        self.mri_set_vox_val(&mut dv, x, y, z, f, q, 0)?;
                    }
                }
            }
        }
        Ok(Some(dv))
    }

    /// `MRISeqchangeType(vol, type, f_low, f_high, no_scale)` (§25): `MRIchangeType` of the
    /// volume seen as one frame of `depth x nframes` slices, the result given back the depth and
    /// frames; a failure's line on standard error and `None`.
    pub fn mri_seq_change_type(
        &mut self,
        vol: &mut Mri,
        to: i32,
        f_low: f32,
        f_high: f32,
        no_scale: i32,
    ) -> Result<Option<Mri>, Stop> {
        let (d, nf) = (vol.geom.depth, vol.nframes);
        reshape(vol, d.wrapping_mul(nf), 1);
        let r = self.mri_change_type(vol, to, f_low, f_high, no_scale);
        reshape(vol, d, nf);
        match r? {
            None => {
                self.put_err(b"ERROR: MRISeqchangeType: MRIchangeType\n");
                Ok(None)
            }
            Some(mut m) => {
                reshape(&mut m, d, nf);
                m.frames = Some(vec![crate::mri::Frame::default(); nf.max(0) as usize]);
                Ok(Some(m))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use larmorx_core::defects::{FS_317, UpstreamDefects};

    use crate::{Environment, Process};

    fn process(mode: UpstreamDefects) -> Process {
        Process::new(
            Environment::new(),
            mode,
            Box::new(std::io::sink()),
            Box::new(std::io::sink()),
        )
    }

    /// FS-317's conversion: numbers inside the `int` range truncated in both modes; NaN and
    /// numbers beyond it -2147483648 with `reproduce`, 0 or the nearer end by default, noted.
    #[test]
    fn fs317_conversions() {
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let mut p = process(mode);
            assert_eq!(p.int_317(-2.9, "t"), -2);
            assert_eq!(p.int_317(2_147_483_647.9, "t"), i32::MAX);
            assert_eq!(p.int_317(-2_147_483_648.9, "t"), i32::MIN);
            assert!(!p.noted(&FS_317));
        }
        let mut p = process(UpstreamDefects::Reproduce);
        for v in [f64::NAN, f64::INFINITY, 3e9, -3e9] {
            assert_eq!(p.int_317(v, "t"), i32::MIN);
        }
        assert!(p.noted(&FS_317));
        let mut p = process(UpstreamDefects::Fix);
        assert_eq!(p.int_317(f64::NAN, "t"), 0);
        assert_eq!(p.int_317(f64::INFINITY, "t"), i32::MAX);
        assert_eq!(p.int_317(3e9, "t"), i32::MAX);
        assert_eq!(p.int_317(-3e9, "t"), i32::MIN);
        assert!(p.noted(&FS_317));
    }
}
