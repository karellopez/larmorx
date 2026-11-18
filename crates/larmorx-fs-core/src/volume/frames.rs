// SPDX-License-Identifier: Apache-2.0
//! Frame-range samplers, the trilinear kernel and trilinear splatting (spec §27):
//! `MRItrilinKernel`, `MRIinterpolateIntoVolume(Frame)`, `MRIsampleSeqVolume(Type)` and the
//! whole-point frame-range samplers; FS-320 and FS-321.
//!
//! The frame-range samplers give their values through a callback `put(frame, value)` (FreeSurfer's
//! array indexed by the frame number itself, or by `frame - begin` for the whole-point samplers):
//! a value never put is the caller's as it was.

use larmorx_core::defects::{FS_298, FS_320, FS_321};

use super::arith::{Elem, elem_size};
use super::sample::{cvtt, interpolated, nearest_type, sample_type};
use crate::mri::{Mri, types};
use crate::process::{Process, Stop, codes};
use crate::sse;

/// The eight trilinear corners of a point and their weights (§27), in the corners' order
/// (x outermost, z innermost: corner `i` takes x's upper index when bit 2 is set, y's when bit 1
/// is, z's when bit 0 is).
#[derive(Clone, Copy, Debug)]
pub struct Corners {
    pub at: [[i32; 3]; 8],
    pub weight: [f64; 8],
}

/// The range of an integer type of a splat (§27), as `i64`.
fn int_range(kind: i32) -> (i64, i64) {
    match kind {
        types::UCHAR => (0, 255),
        types::SHORT => (i64::from(i16::MIN), i64::from(i16::MAX)),
        types::USHRT => (0, i64::from(u16::MAX)),
        types::INT => (i64::from(i32::MIN), i64::from(i32::MAX)),
        _ => (i64::MIN, i64::MAX),
    }
}

/// An integer's low bits as an element of the integer type `kind` (bytes, at most 8).
fn int_le(kind: i32, v: i64) -> ([u8; 8], usize) {
    let mut b = [0u8; 8];
    let n = match kind {
        types::UCHAR => {
            b[0] = v as u8;
            1
        }
        types::SHORT => {
            b[..2].copy_from_slice(&(v as i16).to_le_bytes());
            2
        }
        types::USHRT => {
            b[..2].copy_from_slice(&(v as u16).to_le_bytes());
            2
        }
        types::INT => {
            b[..4].copy_from_slice(&(v as i32).to_le_bytes());
            4
        }
        _ => {
            b.copy_from_slice(&v.to_le_bytes());
            8
        }
    };
    (b, n)
}

impl Process {
    /// The trilinear corners of a point (§27): `None` for a point unambiguously outside (§15's
    /// `MRIindexNotInVolume` answer 1, NaN included); else the point clamped to the extent on each
    /// axis, the truncation and the next index (at most the last), the fractions in `double`.
    pub fn trilinear_corners(&mut self, m: &Mri, x: f64, y: f64, z: f64) -> Option<Corners> {
        if self.mri_index_not_in_volume(m, x, y, z) == 1 {
            return None;
        }
        let e = [m.geom.width, m.geom.height, m.geom.depth];
        let mut lo = [0i32; 3];
        let mut hi = [0i32; 3];
        let mut md = [0f64; 3];
        let mut pd = [0f64; 3];
        for (k, c) in [x, y, z].into_iter().enumerate() {
            let mut c = c;
            if c >= f64::from(e[k]) {
                c = f64::from(e[k] - 1);
            }
            if c < 0.0 {
                c = 0.0;
            }
            lo[k] = cvtt(c).max(0);
            hi[k] = (e[k] - 1).min(lo[k] + 1);
            md[k] = sse::sub64(c, f64::from(lo[k]));
            pd[k] = sse::sub64(1.0, md[k]);
        }
        let mut c = Corners {
            at: [[0; 3]; 8],
            weight: [0.0; 8],
        };
        for i in 0..8 {
            let (bx, by, bz) = (i & 4 != 0, i & 2 != 0, i & 1 != 0);
            c.at[i] = [
                if bx { hi[0] } else { lo[0] },
                if by { hi[1] } else { lo[1] },
                if bz { hi[2] } else { lo[2] },
            ];
            let wx = if bx { md[0] } else { pd[0] };
            let wy = if by { md[1] } else { pd[1] };
            let wz = if bz { md[2] } else { pd[2] };
            c.weight[i] = sse::mul64(sse::mul64(wx, wy), wz);
        }
        Some(c)
    }

    /// `MRItrilinKernel(mri, c, r, s, kernel)` (§27): the eight trilinear weights of the point,
    /// all 0 for a point unambiguously outside; no voxel is read (any type).
    pub fn mri_trilin_kernel(&mut self, m: &Mri, c: f64, r: f64, s: f64) -> [f64; 8] {
        self.trilinear_corners(m, c, r, s)
            .map_or([0.0; 8], |k| k.weight)
    }

    /// `MRIinterpolateIntoVolume(mri, x, y, z, val)` (§27): splatting into frame 0.
    pub fn mri_interpolate_into_volume(
        &mut self,
        m: &mut Mri,
        x: f64,
        y: f64,
        z: f64,
        val: f64,
    ) -> Result<i32, Stop> {
        self.mri_interpolate_into_volume_frame(m, x, y, z, 0, val)
    }

    /// `MRIinterpolateIntoVolumeFrame(mri, x, y, z, frame, val)` (§27): `val` added into the eight
    /// corners of `frame`, each its share `weight * val`; integer voxels get the share rounded
    /// (`nint`), the sum clamped to the type's range by default (FS-320: FreeSurfer's wraps
    /// around); a 64-bit volume adds into the voxel by default (FS-321: FreeSurfer adds into 32-bit
    /// integers at the voxel's index). The frame is not checked (§6's direct access).
    #[allow(clippy::too_many_arguments)]
    pub fn mri_interpolate_into_volume_frame(
        &mut self,
        m: &mut Mri,
        x: f64,
        y: f64,
        z: f64,
        frame: i32,
        val: f64,
    ) -> Result<i32, Stop> {
        let Some(c) = self.trilinear_corners(m, x, y, z) else {
            return Ok(codes::NONE);
        };
        if !interpolated(m.kind) {
            self.error_printf(
                codes::UNSUPPORTED,
                format!("MRIsampleVolume: unsupported type {}", m.kind).as_bytes(),
            );
            return Ok(codes::UNSUPPORTED);
        }
        self.direct_needs_voxels(m, "MRIinterpolateIntoVolumeFrame")?;
        for i in 0..8 {
            let share = sse::mul64(c.weight[i], val);
            self.splat_add(m, c.at[i], frame, share)?;
        }
        Ok(codes::NONE)
    }

    /// One corner's addition of a splat (§27).
    fn splat_add(&mut self, m: &mut Mri, p: [i32; 3], f: i32, share: f64) -> Result<(), Stop> {
        const WHAT: &str = "MRIinterpolateIntoVolumeFrame";
        let (x, y, z, f) = (
            i64::from(p[0]),
            i64::from(p[1]),
            i64::from(p[2]),
            i64::from(f),
        );
        let kind = m.kind;
        if kind == types::FLOAT {
            let place = self.place_of(m, x, y, z, f, 4);
            let b = self.direct_read8(m, place, 4, WHAT);
            let v = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
            let r = sse::narrow(sse::add64(share, sse::widen(v)));
            self.direct_write(m, place, &r.to_le_bytes(), WHAT);
            return Ok(());
        }
        let n = self.nint(share);
        let reproduce = self.mode().reproduce();
        if kind == types::LONG {
            if n != 0 {
                let how = if reproduce {
                    "added into the 32-bit integer at the voxel's index, as FreeSurfer's"
                } else {
                    "added into the voxel (FreeSurfer adds into the 32-bit integer at the voxel's index)"
                };
                self.note(&FS_321, format!("{WHAT}: a 64-bit volume's share {how}"));
            }
            if reproduce {
                let place = self.row_place(m, 4 * x, y, z, f, 4);
                let b = self.direct_read8(m, place, 4, WHAT);
                let v = i32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                let sum = i64::from(v) + i64::from(n);
                if sum < i64::from(i32::MIN) || sum > i64::from(i32::MAX) {
                    self.note(
                        &FS_320,
                        format!(
                            "{WHAT}: a 32-bit sum beyond the range wrapped around, as FreeSurfer's"
                        ),
                    );
                }
                self.direct_write(m, place, &(sum as i32).to_le_bytes(), WHAT);
                return Ok(());
            }
            let place = self.place_of(m, x, y, z, f, 8);
            let b = self.direct_read8(m, place, 8, WHAT);
            let v = i64::from_le_bytes(b);
            let r = v.saturating_add(i64::from(n));
            self.direct_write(m, place, &r.to_le_bytes(), WHAT);
            return Ok(());
        }
        let size = elem_size(kind).expect("an integer type");
        let place = self.place_of(m, x, y, z, f, size);
        let b = self.direct_read8(m, place, size as usize, WHAT);
        let v = match Elem::decode(kind, &b) {
            Elem::U8(v) => i64::from(v),
            Elem::I16(v) => i64::from(v),
            Elem::U16(v) => i64::from(v),
            Elem::I32(v) => i64::from(v),
            _ => 0,
        };
        let (lo, hi) = int_range(kind);
        let mut sum = v + i64::from(n);
        if sum < lo || sum > hi {
            if reproduce {
                self.note(
                    &FS_320,
                    format!(
                        "{WHAT}: a sum beyond the type's range wrapped around, as FreeSurfer's"
                    ),
                );
            } else {
                self.note(
                    &FS_320,
                    format!("{WHAT}: a sum beyond the type's range clamped to it (FreeSurfer's wraps around)"),
                );
                sum = sum.clamp(lo, hi);
            }
        }
        let (bytes, len) = int_le(kind, sum);
        self.direct_write(m, place, &bytes[..len], WHAT);
        Ok(())
    }

    /// `MRIsampleSeqVolume(mri, x, y, z, valvect, first, last)` (§27): the trilinear sample of
    /// each frame `first..=last`, `put(frame, value)`; `outside_val` (as a float) to each frame for
    /// a point unambiguously outside.
    #[allow(clippy::too_many_arguments)]
    pub fn mri_sample_seq_volume(
        &mut self,
        m: &Mri,
        x: f64,
        y: f64,
        z: f64,
        first: i32,
        last: i32,
        put: &mut dyn FnMut(i32, f32),
    ) -> Result<i32, Stop> {
        self.seq_trilinear(m, x, y, z, first, last, put)
    }

    #[allow(clippy::too_many_arguments)]
    fn seq_trilinear(
        &mut self,
        m: &Mri,
        x: f64,
        y: f64,
        z: f64,
        first: i32,
        last: i32,
        put: &mut dyn FnMut(i32, f32),
    ) -> Result<i32, Stop> {
        const NAME: &str = "MRIsampleSeqVolume";
        let Some(c) = self.trilinear_corners(m, x, y, z) else {
            let v = sse::narrow(m.outside_val);
            for f in first..=last {
                put(f, v);
            }
            return Ok(codes::NONE);
        };
        for f in first..=last {
            if !interpolated(m.kind) {
                self.error_printf(
                    codes::UNSUPPORTED,
                    format!("{NAME}: unsupported type {}", m.kind).as_bytes(),
                );
                return Ok(codes::UNSUPPORTED);
            }
            let mut s = 0.0;
            for i in 0..8 {
                let [px, py, pz] = c.at[i];
                let v = self.sampler_read(m, px, py, pz, f, NAME)?;
                let t = sse::mul64(c.weight[i], v);
                s = if i == 0 { t } else { sse::add64(s, t) };
            }
            put(f, sse::narrow(s));
        }
        Ok(codes::NONE)
    }

    /// `MRIsampleSeqVolumeType(mri, x, y, z, valvect, first, last, type)` (§27): trilinear (1;
    /// cubic 3 and sinc 2 after their line), the cubic B-spline (5) ends the program (status 1),
    /// every other code the nearest voxel of each frame.
    #[allow(clippy::too_many_arguments)]
    pub fn mri_sample_seq_volume_type(
        &mut self,
        m: &Mri,
        x: f64,
        y: f64,
        z: f64,
        first: i32,
        last: i32,
        kind: i32,
        put: &mut dyn FnMut(i32, f32),
    ) -> Result<i32, Stop> {
        match kind {
            sample_type::TRILINEAR => self.seq_trilinear(m, x, y, z, first, last, put),
            sample_type::CUBIC | sample_type::SINC => {
                let what = if kind == sample_type::CUBIC {
                    "Cubic"
                } else {
                    "Sinc"
                };
                self.put_out(
                    format!("{what} interpolation is not implemented yet on multi-frame data. Going ahead with tri-linear").as_bytes(),
                );
                self.seq_trilinear(m, x, y, z, first, last, put)
            }
            sample_type::CUBIC_BSPLINE => {
                self.put_out(b"Cubic Bspline interpolation not implemented here. Needs to be done by calling function!");
                self.flush();
                Err(Stop::Exit(1))
            }
            _ => self.seq_nearest(m, x, y, z, first, last, put),
        }
    }

    /// The nearest voxel of each frame (§27's `MRIsampleSeqVolumeType` with another code).
    #[allow(clippy::too_many_arguments)]
    fn seq_nearest(
        &mut self,
        m: &Mri,
        x: f64,
        y: f64,
        z: f64,
        first: i32,
        last: i32,
        put: &mut dyn FnMut(i32, f32),
    ) -> Result<i32, Stop> {
        const NAME: &str = "MRIsampleSeqVolumeType";
        if self.mri_index_not_in_volume(m, x, y, z) == 1 {
            let v = sse::narrow(m.outside_val);
            for f in first..=last {
                put(f, v);
            }
            return Ok(codes::NONE);
        }
        let e = [m.geom.width, m.geom.height, m.geom.depth];
        let p = [x, y, z];
        let mut q = [0i32; 3];
        for k in 0..3 {
            q[k] = self.nint(p[k]).max(0).min(e[k] - 1);
        }
        for f in first..=last {
            if !interpolated(m.kind) {
                put(f, 0.0);
                self.error_printf(
                    codes::UNSUPPORTED,
                    format!("{NAME}: unsupported type {}", m.kind).as_bytes(),
                );
                return Ok(codes::UNSUPPORTED);
            }
            let v = if m.kind == types::LONG {
                self.long_nearest(m, q, f, NAME)?
            } else {
                self.nearest_voxel(m, q, f, NAME)
            };
            put(f, v);
        }
        Ok(codes::NONE)
    }

    /// A 64-bit voxel read by a nearest sampler as a float (FS-298, §18): the voxel's value by
    /// default; FreeSurfer's 32-bit integer at the voxel's index with `reproduce`.
    fn long_nearest(&mut self, m: &Mri, q: [i32; 3], f: i32, what: &str) -> Result<f32, Stop> {
        if self.mode().reproduce() {
            let v = self.sampler_read(m, q[0], q[1], q[2], f, what)?;
            return Ok(sse::narrow(v));
        }
        if !self.noted(&FS_298) {
            self.note(
                &FS_298,
                format!("{what}: a 64-bit volume sampled at its voxels' values"),
            );
        }
        self.direct_needs_voxels(m, what)?;
        let place = self.place_of(
            m,
            i64::from(q[0]),
            i64::from(q[1]),
            i64::from(q[2]),
            i64::from(f),
            8,
        );
        let b = self.direct_read8(m, place, 8, what);
        Ok(i64::from_le_bytes(b) as f32)
    }

    /// `MRIsampleVolumeFrame_xyzInt_nRange_floats(mri, x, y, z, begin, end, vals)` and
    /// `MRIsampleVolumeFrameType_xyzInt_nRange_SAMPLE_NEAREST_floats` (§27): the voxel at the
    /// whole point (clamped to the volume) of each frame `begin..end`, `put(frame - begin,
    /// value)`; `outside_val` for a point unambiguously outside; bytes, shorts, unsigned shorts,
    /// ints and floats only (another type 0 for every frame and an error).
    #[allow(clippy::too_many_arguments)]
    pub fn mri_sample_volume_frame_xyz_int_n_range_floats(
        &mut self,
        m: &Mri,
        x: i32,
        y: i32,
        z: i32,
        begin: i32,
        end: i32,
        put: &mut dyn FnMut(i32, f32),
    ) -> Result<i32, Stop> {
        const NAME: &str = "MRIsampleVolumeFrameType";
        let p = [f64::from(x), f64::from(y), f64::from(z)];
        if self.mri_index_not_in_volume(m, p[0], p[1], p[2]) == 1 {
            let v = sse::narrow(m.outside_val);
            for f in begin..end {
                put(f - begin, v);
            }
            return Ok(codes::NONE);
        }
        let e = [m.geom.width, m.geom.height, m.geom.depth];
        let mut q = [x, y, z];
        for k in 0..3 {
            q[k] = q[k].max(0).min(e[k] - 1);
        }
        if !nearest_type(m.kind) {
            for f in begin..end {
                put(f - begin, 0.0);
            }
            self.error_printf(
                codes::UNSUPPORTED,
                format!("{NAME}: unsupported volume type {}", m.kind).as_bytes(),
            );
            return Ok(codes::UNSUPPORTED);
        }
        for f in begin..end {
            let v = self.nearest_voxel(m, q, f, NAME);
            put(f - begin, v);
        }
        Ok(codes::NONE)
    }
}
