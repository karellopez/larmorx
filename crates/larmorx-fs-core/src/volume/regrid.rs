// SPDX-License-Identifier: Apache-2.0
//! Resampling onto a template's grid and replacing a value (spec §31): `MRIreplaceValues`,
//! `MRIreplaceValuesUchar`, `MRIresample` and `MRIresampleFill` (the samplers, the vote, the
//! geometry); FS-326 to FS-328 and FS-461.

use larmorx_core::defects::{FS_298, FS_326, FS_327, FS_328, FS_461};

use super::sample::{FLT_EPSILON, cvtt, sample_type};
use super::{Dst, VolOut, Vols};
use crate::clib::printf::sprintf_f64;
use crate::maths::Returned;
use crate::maths::matrix::{MATRIX_REAL, Matrix, real_point_f};
use crate::mri::geom::DIAG_SHOW;
use crate::mri::{Mri, types};
use crate::process::{DIAG_VERBOSE, Process, Stop, codes};
use crate::sse;

/// The vote's code (§31).
pub const SAMPLE_VOTE: i32 = 6;
/// The weighted sampler's code (§31).
pub const SAMPLE_WEIGHTED: i32 = 4;

/// The types `MRIresampleFill`'s samplers read and write (§31).
fn sampled(kind: i32) -> bool {
    matches!(
        kind,
        types::UCHAR | types::SHORT | types::USHRT | types::INT | types::LONG | types::FLOAT
    )
}

/// §8.1's float product of a matrix and the point `(x, y, z, 1)` into the 4 x 1 matrix `out`
/// (whose values stay where the product fails); its first three values.
fn apply(
    p: &mut Process,
    m: &Matrix,
    q: [f32; 3],
    v: &mut Matrix,
    out: &mut Matrix,
) -> Result<[f32; 3], Stop> {
    if !m.is_complex() && m.rows == 4 && m.cols == 4 {
        let r = real_point_f(m, [q[0], q[1], q[2], 1.0]);
        return Ok([r[0], r[1], r[2]]);
    }
    for (k, x) in q.iter().enumerate() {
        v.set(k as i32 + 1, 1, *x);
    }
    p.matrix_multiply(Some(m), Some(v), Some(out))?;
    Ok([out.at(1, 1), out.at(2, 1), out.at(3, 1)])
}

impl Process {
    /// The size check of `MRIreplaceValues` (§31, `MRIcheckVolDims`): another height, width or
    /// depth ends the program (code -4).
    fn check_vol_dims(&mut self, a: &Mri, b: &Mri) -> Result<(), Stop> {
        for (what, x, y) in [
            ("height", a.geom.height, b.geom.height),
            ("width", a.geom.width, b.geom.width),
            ("depth", a.geom.depth, b.geom.depth),
        ] {
            if x != y {
                let mut msg = b"ERROR: ".to_vec();
                msg.extend_from_slice(&self.progname);
                msg.extend_from_slice(
                    format!("-MRIcheckVolDims: volume1 {what}={x} != volume2 {what}={y}.\n")
                        .as_bytes(),
                );
                self.error_exit(codes::BAD_PARM, &msg)?;
            }
        }
        Ok(())
    }

    /// `MRIreplaceValues(src, dst, in_val, out_val)` (§31): every voxel within `FLT_EPSILON` of
    /// `in_val` becomes `out_val`, the others copied (as the voxel access converts them); byte
    /// volumes go to [`Process::mri_replace_values_uchar`].
    pub fn mri_replace_values(
        &mut self,
        src: &mut Mri,
        dst: Dst,
        in_val: f32,
        out_val: f32,
    ) -> Result<VolOut, Stop> {
        let mut v = Vols::new(src, None, dst);
        if v.has_dst() {
            self.check_vol_dims(v.a(), v.d())?;
        } else {
            let c = self.mri_clone_new(v.a())?;
            v.set_dst(c);
        }
        if v.a().kind == types::UCHAR && v.d().kind == types::UCHAR {
            let a = self.nint(sse::widen(in_val)) as u8;
            let b = self.nint(sse::widen(out_val)) as u8;
            self.replace_bytes(&mut v, a, b)?;
        } else {
            self.replace_any(&mut v, in_val, out_val)?;
        }
        Ok(v.out())
    }

    /// `MRIreplaceValuesUchar(src, dst, in_byte, out_byte)` (§31): each byte equal to `in_byte`
    /// becomes `out_byte` (FS-461: FreeSurfer's in frame 0 only); volumes that are not bytes go to
    /// [`Process::mri_replace_values`] with the bytes as floats.
    pub fn mri_replace_values_uchar(
        &mut self,
        src: &mut Mri,
        dst: Dst,
        in_byte: u8,
        out_byte: u8,
    ) -> Result<VolOut, Stop> {
        let mut v = Vols::new(src, None, dst);
        if v.has_dst() {
            self.check_vol_dims(v.a(), v.d())?;
        } else {
            let c = self.mri_clone_new(v.a())?;
            v.set_dst(c);
        }
        if v.a().kind != types::UCHAR || v.d().kind != types::UCHAR {
            self.replace_any(&mut v, f32::from(in_byte), f32::from(out_byte))?;
        } else {
            self.replace_bytes(&mut v, in_byte, out_byte)?;
        }
        Ok(v.out())
    }

    /// The general replacement (§31): slices, rows, columns, frames innermost.
    fn replace_any(&mut self, v: &mut Vols, in_val: f32, out_val: f32) -> Result<(), Stop> {
        let (w, h, d, nf) = (
            v.a().geom.width,
            v.a().geom.height,
            v.a().geom.depth,
            v.a().nframes,
        );
        let eps = FLT_EPSILON as f32;
        // two float volumes with every voxel inside: element by element (the voxel access reads and
        // writes a float as it is)
        let n =
            (w.max(0) as usize) * (h.max(0) as usize) * (d.max(0) as usize) * (nf.max(0) as usize);
        let floats = |m: &Mri| {
            m.kind == types::FLOAT
                && m.nframes >= nf
                && m.voxels.as_ref().is_some_and(|x| x.len() >= 4 * n)
        };
        if floats(v.a()) && floats(v.d()) {
            let swap = |b: &[u8]| {
                let x = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                if sse::sub32(x, in_val).abs() < eps {
                    out_val
                } else {
                    x
                }
            };
            if v.d_is_a() {
                if let Some(x) = v.d_mut().voxels.as_deref_mut() {
                    for c in x[..4 * n].as_chunks_mut::<4>().0 {
                        let y = swap(c);
                        c.copy_from_slice(&y.to_le_bytes());
                    }
                }
            } else {
                let src: Vec<u8> = v
                    .a()
                    .voxels
                    .as_deref()
                    .map_or(Vec::new(), |x| x[..4 * n].to_vec());
                if let Some(x) = v.d_mut().voxels.as_deref_mut() {
                    for (o, c) in x[..4 * n]
                        .as_chunks_mut::<4>()
                        .0
                        .iter_mut()
                        .zip(src.as_chunks::<4>().0)
                    {
                        o.copy_from_slice(&swap(c).to_le_bytes());
                    }
                }
            }
            return Ok(());
        }
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    for f in 0..nf {
                        let mut val = self.mri_get_vox_val(v.a(), x, y, z, f, 0)?;
                        if sse::sub32(val, in_val).abs() < eps {
                            val = out_val;
                        }
                        self.mri_set_vox_val(v.d_mut(), x, y, z, f, val, 0)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// The byte replacement (§31): frame 0 (FS-461's `reproduce`) or every frame (the default).
    fn replace_bytes(&mut self, v: &mut Vols, a: u8, b: u8) -> Result<(), Stop> {
        let (w, h, d) = (v.a().geom.width, v.a().geom.height, v.a().geom.depth);
        let nf = v.a().nframes.min(v.d().nframes);
        let frames = if v.a().nframes > 1 || v.d().nframes > 1 {
            if self.mode().reproduce() {
                self.note(
                    &FS_461,
                    "MRIreplaceValuesUchar: frame 0 only, as FreeSurfer's",
                );
                1
            } else {
                self.note(
                    &FS_461,
                    "MRIreplaceValuesUchar: every frame (FreeSurfer's replaces in frame 0 only)",
                );
                nf
            }
        } else {
            1
        };
        if w <= 0 || h <= 0 || d <= 0 {
            return Ok(());
        }
        self.direct_needs_voxels(v.a(), "MRIreplaceValuesUchar")?;
        self.direct_needs_voxels(v.d(), "MRIreplaceValuesUchar")?;
        // every byte inside both volumes: one walk of the buffers (the same bytes)
        let n = (w as usize) * (h as usize) * (d as usize) * (frames.max(1) as usize);
        let fits =
            |m: &Mri| m.nframes >= frames.max(1) && m.voxels.as_ref().is_some_and(|x| x.len() >= n);
        if fits(v.a()) && fits(v.d()) {
            if v.d_is_a() {
                if let Some(x) = v.d_mut().voxels.as_deref_mut() {
                    for c in &mut x[..n] {
                        if *c == a {
                            *c = b;
                        }
                    }
                }
            } else {
                let src: Vec<u8> = v
                    .a()
                    .voxels
                    .as_deref()
                    .map_or(Vec::new(), |x| x[..n].to_vec());
                if let Some(x) = v.d_mut().voxels.as_deref_mut() {
                    for (o, &c) in x[..n].iter_mut().zip(&src) {
                        *o = if c == a { b } else { c };
                    }
                }
            }
            return Ok(());
        }
        for f in 0..frames.max(1) {
            for z in 0..d {
                for y in 0..h {
                    for x in 0..w {
                        let (xi, yi, zi, fi) =
                            (i64::from(x), i64::from(y), i64::from(z), i64::from(f));
                        let place = self.place_of(v.a(), xi, yi, zi, fi, 1);
                        let s = self.direct_read8(v.a(), place, 1, "MRIreplaceValuesUchar")[0];
                        let o = if s == a { b } else { s };
                        let place = self.place_of(v.d(), xi, yi, zi, fi, 1);
                        self.direct_write(v.d_mut(), place, &[o], "MRIreplaceValuesUchar");
                    }
                }
            }
        }
        Ok(())
    }

    /// `MRIresample(src, template, type)` (§31): `MRIresampleFill` with the fill 0.
    pub fn mri_resample(
        &mut self,
        src: &Mri,
        template: &Mri,
        kind: i32,
    ) -> Result<Option<Mri>, Stop> {
        self.mri_resample_fill(src, template, kind, 0.0)
    }

    /// `MRIresampleFill(src, template, type, fill)` (§31): the source sampled on the template's
    /// grid (the template's extent and header, the source's type, the fewer frames), every voxel
    /// first `fill`; the vote (6) takes the most frequent label (FS-326, FS-327); the geometry
    /// from the source's matrix and the resampling matrix.
    pub fn mri_resample_fill(
        &mut self,
        src: &Mri,
        template: &Mri,
        kind: i32,
        fill: f32,
    ) -> Result<Option<Mri>, Stop> {
        if src.geom.ras_good_flag == 0 {
            self.put_out(
                b"MRIresample(): WARNING: ras_good_flag is not set, changing orientation\nto default.\n",
            );
        }
        let Some(m) = self.mri_get_resample_matrix(src, template)? else {
            return Ok(None);
        };
        let show = self.gdiag & DIAG_SHOW != 0 && self.gdiag & DIAG_VERBOSE != 0;
        if show {
            self.put_out(b"MRIresample() matrix is:\n");
            self.matrix_print(Some(false), Some(&m), 5);
        }
        let tg = &template.geom;
        let nf = src.nframes.min(template.nframes);
        let mut dest = self.mri_alloc_sequence(tg.width, tg.height, tg.depth, src.kind, nf)?;
        self.mri_replace_values(&mut dest, Dst::Src, 0.0, fill)?;
        self.copy_header_into(template, &mut dest)?;
        self.copy_pulse_parameters(src, &mut dest);
        if kind == sample_type::CUBIC_BSPLINE {
            self.error_exit(
                codes::UNSUPPORTED,
                b"MRIresampleFill: cubic B-spline sampling needs fs.bspline (not yet ported)\n",
            )?;
            return Ok(None);
        }
        if kind == SAMPLE_VOTE {
            if !self.vote(src, template, &mut dest)? {
                return Ok(None);
            }
        } else {
            self.resample_samples(src, &m, &mut dest, kind)?;
        }
        self.resample_geometry(src, &m, &mut dest, show)?;
        Ok(Some(dest))
    }

    /// `MRIresampleFill`'s sampling (§31): every voxel of the destination's frames, the slice
    /// innermost, at `M (di, dj, dk, 1)` in the source.
    fn resample_samples(
        &mut self,
        src: &Mri,
        m: &Matrix,
        dest: &mut Mri,
        kind: i32,
    ) -> Result<(), Stop> {
        let (w, h, d, nf) = (
            dest.geom.width,
            dest.geom.height,
            dest.geom.depth,
            dest.nframes,
        );
        let mut vx = Matrix::zeros(4, 1, MATRIX_REAL);
        let mut vy = Matrix::zeros(4, 1, MATRIX_REAL);
        vx.set(4, 1, 1.0);
        let reproduce = self.mode().reproduce();
        let mut value = 0.0f32;
        let mut sample = 0.0f64;
        for f in 0..nf {
            for di in 0..w {
                for dj in 0..h {
                    for dk in 0..d {
                        let sp =
                            apply(self, m, [di as f32, dj as f32, dk as f32], &mut vx, &mut vy)?;
                        match kind {
                            sample_type::SINC | sample_type::CUBIC => {
                                let (x, y, z) =
                                    (sse::widen(sp[0]), sse::widen(sp[1]), sse::widen(sp[2]));
                                if kind == sample_type::SINC {
                                    self.mri_sinc_sample_volume_frame(
                                        src,
                                        x,
                                        y,
                                        z,
                                        f,
                                        5,
                                        &mut sample,
                                    )?;
                                } else {
                                    self.mri_cubic_sample_volume_frame(
                                        src,
                                        x,
                                        y,
                                        z,
                                        f,
                                        &mut sample,
                                    )?;
                                }
                                value = sse::narrow(sample);
                            }
                            sample_type::NEAREST | sample_type::TRILINEAR | SAMPLE_WEIGHTED => {
                                let mut base = [0i32; 3];
                                let mut frac = [0f32; 3];
                                for k in 0..3 {
                                    base[k] = cvtt(f64::from(sp[k]).floor());
                                    frac[k] = sse::sub32(sp[k], base[k] as f32);
                                }
                                let digit = |x: f32| {
                                    let below = x < 0.5;
                                    i32::from(!below)
                                };
                                if kind == sample_type::NEAREST && src.kind != types::LONG {
                                    let q = [
                                        base[0].wrapping_add(digit(frac[0])),
                                        base[1].wrapping_add(digit(frac[1])),
                                        base[2].wrapping_add(digit(frac[2])),
                                    ];
                                    value = self.corner_value(src, q, f);
                                } else {
                                    let c = self.corner_values(src, base, f)?;
                                    value = match kind {
                                        sample_type::TRILINEAR => trilinear(frac, &c),
                                        sample_type::NEAREST => {
                                            let i = digit(frac[0]) * 4
                                                + digit(frac[1]) * 2
                                                + digit(frac[2]);
                                            c[i as usize]
                                        }
                                        _ => weighted(frac, &c),
                                    };
                                }
                            }
                            _ => {}
                        }
                        self.store_resampled(dest, [di, dj, dk], f, value, reproduce)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// One corner of a sampled point (§31) of a source that is not 64-bit: 0 outside the source
    /// (and for a type it does not read), else the voxel as a float.
    fn corner_value(&mut self, src: &Mri, q: [i32; 3], f: i32) -> f32 {
        let e = [src.geom.width, src.geom.height, src.geom.depth];
        if !sampled(src.kind) || (0..3).any(|k| q[k] < 0 || q[k] >= e[k]) {
            return 0.0;
        }
        if let Some(v) = super::sample::inside_elem(src, q[0], q[1], q[2], f, false) {
            return v.to_f32();
        }
        self.nearest_voxel(src, q, f, "MRIresample")
    }

    /// The eight corners `val[a][b][c]` of a sampled point (§31): 0 outside the source (and for a
    /// type it does not read), else the voxel as a float (FS-298 for a 64-bit source).
    fn corner_values(&mut self, src: &Mri, base: [i32; 3], f: i32) -> Result<[f32; 8], Stop> {
        let mut c = [0f32; 8];
        if !sampled(src.kind) {
            return Ok(c);
        }
        let e = [src.geom.width, src.geom.height, src.geom.depth];
        for (i, slot) in c.iter_mut().enumerate() {
            let q = [
                base[0].wrapping_add((i >> 2) as i32 & 1),
                base[1].wrapping_add((i >> 1) as i32 & 1),
                base[2].wrapping_add(i as i32 & 1),
            ];
            if (0..3).any(|k| q[k] < 0 || q[k] >= e[k]) {
                continue;
            }
            *slot = if src.kind == types::LONG {
                if self.mode().reproduce() {
                    sse::narrow(self.sampler_read(src, q[0], q[1], q[2], f, "MRIresample")?)
                } else {
                    if !self.noted(&FS_298) {
                        self.note(
                            &FS_298,
                            "MRIresample: a 64-bit volume sampled at its voxels' values",
                        );
                    }
                    let place = self.place_of(
                        src,
                        i64::from(q[0]),
                        i64::from(q[1]),
                        i64::from(q[2]),
                        i64::from(f),
                        8,
                    );
                    let b = self.direct_read8(src, place, 8, "MRIresample");
                    i64::from_le_bytes(b) as f32
                }
            } else {
                self.nearest_voxel(src, q, f, "MRIresample")
            };
        }
        Ok(c)
    }

    /// A resampled value stored in the destination's type (§31): clipped to the type's range,
    /// rounded (`nint`); a float as it is; FS-328 for a 64-bit destination; other types not
    /// written.
    fn store_resampled(
        &mut self,
        dest: &mut Mri,
        p: [i32; 3],
        f: i32,
        value: f32,
        reproduce: bool,
    ) -> Result<(), Stop> {
        const WHAT: &str = "MRIresample";
        let (x, y, z, fi) = (
            i64::from(p[0]),
            i64::from(p[1]),
            i64::from(p[2]),
            i64::from(f),
        );
        let clip = |v: f32, lo: f32, hi: f32| {
            let mut v = v;
            if v < lo {
                v = lo;
            }
            if v > hi {
                v = hi;
            }
            v
        };
        let (bytes, n): ([u8; 4], usize) = match dest.kind {
            types::UCHAR => {
                let v = clip(value, 0.0, 255.0);
                ([self.nint(sse::widen(v)) as u8, 0, 0, 0], 1)
            }
            types::SHORT => {
                let v = clip(value, -32768.0, 32767.0);
                let b = (self.nint(sse::widen(v)) as i16).to_le_bytes();
                ([b[0], b[1], 0, 0], 2)
            }
            types::USHRT => {
                let v = clip(value, 0.0, 65535.0);
                let b = (self.nint(sse::widen(v)) as u16).to_le_bytes();
                ([b[0], b[1], 0, 0], 2)
            }
            types::INT => {
                let v = clip(value, i32::MIN as f32, i32::MAX as f32);
                (self.nint(sse::widen(v)).to_le_bytes(), 4)
            }
            types::LONG => {
                if reproduce {
                    self.note(
                        &FS_328,
                        "MRIresample: a 64-bit voxel written as a 32-bit integer at the voxel's index, as FreeSurfer's",
                    );
                    let v = clip(value, i64::MIN as f32, i64::MAX as f32);
                    let n = self.nint(sse::widen(v));
                    let place = self.row_place(dest, 4 * x, y, z, fi, 4);
                    self.direct_write(dest, place, &n.to_le_bytes(), WHAT);
                    return Ok(());
                }
                self.note(
                    &FS_328,
                    "MRIresample: a 64-bit voxel written whole (FreeSurfer writes a 32-bit integer at the voxel's index)",
                );
                self.mri_set_vox_val(dest, p[0], p[1], p[2], f, value, 0)?;
                return Ok(());
            }
            types::FLOAT => (value.to_le_bytes(), 4),
            _ => return Ok(()),
        };
        let place = self.place_of(dest, x, y, z, fi, n as u64);
        if let (crate::mri::voxels::Place::In(o), Some(v)) = (place, dest.voxels.as_deref_mut()) {
            v[o as usize..o as usize + n].copy_from_slice(&bytes[..n]);
            return Ok(());
        }
        self.direct_write(dest, place, &bytes[..n], WHAT);
        Ok(())
    }

    /// The vote (§31): each source voxel's label counted at the template voxel it falls in; each
    /// template voxel the label with the most votes (the lowest on ties). FS-326: FreeSurfer's
    /// counts stop at 255; FS-327: no matrix from the template to the source refused. `false`
    /// when the destination is dropped.
    fn vote(&mut self, src: &Mri, template: &Mri, dest: &mut Mri) -> Result<bool, Stop> {
        self.put_out(b"resampling using voting...\n");
        let tg = &template.geom;
        let (tw, th, td) = (tg.width, tg.height, tg.depth);
        let Some(mut a) = self.mri_get_resample_matrix(template, src)? else {
            self.note(
                &FS_327,
                "MRIresampleFill: no matrix from the template to the source for the vote: refused (FreeSurfer crashes)",
            );
            self.error_printf(
                codes::BAD_PARM,
                b"MRIresampleFill: no matrix from the template to the source for the vote",
            );
            return Ok(false);
        };
        for r in 1..=3 {
            let t = sse::narrow(sse::add64(sse::widen(a.at(r, 4)), 0.5));
            a.set(r, 4, t);
        }
        let reproduce = self.mode().reproduce();
        let cells = (tw.max(0) as usize) * (th.max(0) as usize) * (td.max(0) as usize);
        // the counts in bytes, as FreeSurfer's (its memory); by default the counts past 255 in a
        // map beside them
        let mut counts = vec![0u8; cells * 256];
        let mut more: std::collections::HashMap<usize, u32> = std::collections::HashMap::new();
        let at = |x: i32, y: i32, z: i32| {
            ((z as usize * th as usize + y as usize) * tw as usize + x as usize) * 256
        };
        let mut vx = Matrix::zeros(4, 1, MATRIX_REAL);
        let mut vy = Matrix::zeros(4, 1, MATRIX_REAL);
        vx.set(4, 1, 1.0);
        let sg = &src.geom;
        let mut capped = false;
        for f in 0..src.nframes {
            for si in 0..sg.width {
                for sj in 0..sg.height {
                    for sk in 0..sg.depth {
                        let t = apply(
                            self,
                            &a,
                            [si as f32, sj as f32, sk as f32],
                            &mut vx,
                            &mut vy,
                        )?;
                        let q = [
                            cvtt(f64::from(t[0]).floor()),
                            cvtt(f64::from(t[1]).floor()),
                            cvtt(f64::from(t[2]).floor()),
                        ];
                        if q[0] < 0
                            || q[0] >= tw
                            || q[1] < 0
                            || q[1] >= th
                            || q[2] < 0
                            || q[2] >= td
                        {
                            continue;
                        }
                        let v = self.mri_get_vox_val(src, si, sj, sk, f, 0)?;
                        let label = self.nint(sse::widen(v)).clamp(0, 255) as usize;
                        let i = at(q[0], q[1], q[2]) + label;
                        if counts[i] == 255 {
                            capped = true;
                            if !reproduce {
                                *more.entry(i).or_insert(255) += 1;
                            }
                        } else {
                            counts[i] += 1;
                        }
                    }
                }
            }
        }
        if capped {
            if reproduce {
                self.note(
                    &FS_326,
                    "MRIresampleFill: the vote's counts stop at 255 (a byte), as FreeSurfer's",
                );
            } else {
                self.note(
                    &FS_326,
                    "MRIresampleFill: every vote counted (FreeSurfer's counts stop at 255)",
                );
            }
        }
        for f in 0..template.nframes {
            for x in 0..tw {
                for y in 0..th {
                    for z in 0..td {
                        let o = at(x, y, z);
                        let count = |l: usize| {
                            let n = counts[o + l];
                            if n == 255 && !more.is_empty() {
                                more.get(&(o + l)).copied().unwrap_or(255)
                            } else {
                                u32::from(n)
                            }
                        };
                        let (mut best, mut most) = (0usize, count(0));
                        for l in 1..256 {
                            let n = count(l);
                            if n > most {
                                best = l;
                                most = n;
                            }
                        }
                        self.mri_set_vox_val(dest, x, y, z, f, best as f32, 0)?;
                    }
                }
            }
        }
        Ok(true)
    }

    /// The destination's geometry after `MRIresampleFill`'s sampling (§31): the cosines from
    /// `i_to_r(src) M` over the voxel sizes, the centre through both matrices; the diagnostic
    /// lines of RAS (0, 0, 0).
    fn resample_geometry(
        &mut self,
        src: &Mri,
        m: &Matrix,
        dest: &mut Mri,
        show: bool,
    ) -> Result<(), Stop> {
        let s2r = self.extract_i_to_r(&src.geom)?;
        let v = match self.matrix_multiply(Some(&s2r), Some(m), None)? {
            Returned::New(v) => v,
            _ => Matrix::zeros(4, 4, MATRIX_REAL),
        };
        let size = [dest.geom.xsize, dest.geom.ysize, dest.geom.zsize];
        let mut cols = [[0f32; 3]; 3];
        for (c, col) in cols.iter_mut().enumerate() {
            for (r, x) in col.iter_mut().enumerate() {
                *x = sse::div32(v.at(r as i32 + 1, c as i32 + 1), size[c]);
            }
        }
        dest.geom.x = cols[0];
        dest.geom.y = cols[1];
        dest.geom.z = cols[2];
        let mut half = Matrix::zeros(4, 1, MATRIX_REAL);
        let ext = [dest.geom.width, dest.geom.height, dest.geom.depth];
        for (k, e) in ext.iter().enumerate() {
            half.set(k as i32 + 1, 1, sse::narrow(f64::from(*e) / 2.0));
        }
        half.set(4, 1, 1.0);
        let p = match self.matrix_multiply(Some(m), Some(&half), None)? {
            Returned::New(p) => p,
            _ => Matrix::zeros(4, 1, MATRIX_REAL),
        };
        let c = match self.matrix_multiply(Some(&s2r), Some(&p), None)? {
            Returned::New(c) => c,
            _ => Matrix::zeros(4, 1, MATRIX_REAL),
        };
        dest.geom.c = [c.at(1, 1), c.at(2, 1), c.at(3, 1)];
        dest.geom.ras_good_flag = 1;
        if show {
            let mut o = Matrix::zeros(4, 1, MATRIX_REAL);
            o.set(4, 1, c.at(4, 1));
            for (label, g) in [("old", src.geom.clone()), ("new", dest.geom.clone())] {
                let i2r = self.extract_i_to_r(&g)?;
                let inv = self.inverse_of(&i2r)?;
                let r = match self.matrix_multiply(inv.as_ref(), Some(&o), None)? {
                    Returned::New(r) => r,
                    _ => Matrix::zeros(4, 1, MATRIX_REAL),
                };
                let f = |x: f32| {
                    String::from_utf8_lossy(&sprintf_f64(b"%2.0f", sse::widen(x))).into_owned()
                };
                self.put_out(
                    format!(
                        "{label} RAS (0,0,0) -> ({}, {}, {})\n",
                        f(r.at(1, 1)),
                        f(r.at(2, 1)),
                        f(r.at(3, 1))
                    )
                    .as_bytes(),
                );
            }
        }
        Ok(())
    }
}

/// §31's trilinear sum of the eight corners (`val[a][b][c]` at `c[4a + 2b + c]`), with the
/// official build's operand orders (the left operand's NaN kept).
fn trilinear(frac: [f32; 3], c: &[f32; 8]) -> f32 {
    use sse::{mul32, mul64, narrow, sub64, widen};
    let (si, sj, sk) = (frac[0], frac[1], frac[2]);
    let a = sub64(1.0, widen(si));
    let b = sub64(1.0, widen(sj));
    let cc = sub64(1.0, widen(sk));
    let v = |i: usize| widen(c[i]);
    let ab = mul64(a, b);
    let t1 = mul64(mul64(ab, cc), v(0));
    let t2 = mul64(mul64(ab, widen(sk)), v(1));
    let ja = mul64(widen(sj), a);
    let t3 = mul64(mul64(cc, ja), v(2));
    let t4 = mul64(mul64(ja, widen(sk)), v(3));
    let bi = mul64(b, widen(si));
    let t5 = mul64(mul64(cc, bi), v(4));
    let t6 = mul64(mul64(widen(sk), bi), v(5));
    let ji = mul32(sj, si);
    let t7 = mul64(mul64(cc, widen(ji)), v(6));
    let t8 = widen(mul32(mul32(ji, sk), c[7]));
    let mut s = t1;
    for t in [t2, t3, t4, t5, t6, t7, t8] {
        s = sse::add64(s, t);
    }
    narrow(s)
}

/// §31's weighted sampler: each corner's weight the sum of its axes' weights; equal values pool
/// their weights at the first such corner; the corner with the largest total gives the value.
fn weighted(frac: [f32; 3], c: &[f32; 8]) -> f32 {
    use sse::{add32, div32, mul32, narrow, sub32, widen};
    let mut ii = [0f32; 3];
    let mut is = [0f32; 3];
    for k in 0..3 {
        let f = frac[k];
        let f2 = mul32(f, f);
        let den = add32(sub32(1.0, mul32(2.0, f)), f2);
        ii[k] = narrow(sse::div64(1.0, widen(den)));
        is[k] = div32(1.0, f2);
    }
    let mut total = [0f32; 8];
    for i in 0..8 {
        let d = [(i >> 2) & 1, (i >> 1) & 1, i & 1];
        let w = |k: usize| if d[k] == 0 { ii[k] } else { is[k] };
        let wi = add32(add32(w(0), w(1)), w(2));
        let first = (0..i).find(|&j| c[j] == c[i]).unwrap_or(i);
        total[first] = add32(total[first], wi);
    }
    let mut best = 0usize;
    for i in 1..8 {
        if total[i] > total[best] {
            best = i;
        }
    }
    c[best]
}
