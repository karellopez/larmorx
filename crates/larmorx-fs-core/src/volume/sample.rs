// SPDX-License-Identifier: Apache-2.0
//! Sampling a volume at a point (spec §18): the nearest, trilinear, cubic and windowed-sinc
//! samplers of frame 0 and of a frame, the two dispatchers by interpolation code, the cubic
//! weights and the sinc kernel; FS-298 to FS-300 and FS-302.
//!
//! A point is given in voxel coordinates (`double`); a sampler's value is written into `*pval`
//! (here `pval`), which keeps the caller's value when the sampler fails before setting it.

use larmorx_core::defects::{FS_290, FS_298, FS_299, FS_300, FS_302};
use larmorx_core::math;

use super::arith::Elem;
use crate::mri::{Mri, types};
use crate::process::{Process, Stop, codes};
use crate::sse;

/// The interpolation codes (§18).
pub mod sample_type {
    pub const NEAREST: i32 = 0;
    pub const TRILINEAR: i32 = 1;
    pub const SINC: i32 = 2;
    pub const CUBIC: i32 = 3;
    pub const CUBIC_BSPLINE: i32 = 5;
}

/// `FLT_EPSILON` as a double.
const FLT_EPSILON: f64 = 1.192_092_895_507_812_5e-7;

/// C's `(int) c` on x86-64 (`cvttsd2si`): truncation; NaN and values outside `int` give
/// -2147483648.
fn cvtt(c: f64) -> i32 {
    if c > -2_147_483_649.0 && c < 2_147_483_648.0 {
        c.trunc() as i32
    } else {
        i32::MIN
    }
}

/// Whether a coordinate is within `FLT_EPSILON` of its integer (§18; NaN and values outside
/// `int` never are).
fn whole(c: f64) -> bool {
    sse::sub64(f64::from(cvtt(c)), c).abs() < FLT_EPSILON
}

/// Whether a point is whole on all three axes.
fn whole3(x: f64, y: f64, z: f64) -> bool {
    whole(x) && whole(y) && whole(z)
}

/// The types the trilinear and cubic samplers read (§18): bytes, shorts, unsigned shorts, ints,
/// 64-bit integers (FS-298) and floats.
fn interpolated(kind: i32) -> bool {
    matches!(
        kind,
        types::UCHAR | types::SHORT | types::USHRT | types::INT | types::LONG | types::FLOAT
    )
}

/// The types the nearest sampler reads (§15).
pub(crate) fn nearest_type(kind: i32) -> bool {
    matches!(
        kind,
        types::UCHAR | types::SHORT | types::USHRT | types::INT | types::FLOAT
    )
}

/// The element as a double (C's conversion).
fn elem_f64(e: Elem) -> f64 {
    match e {
        Elem::U8(v) => f64::from(v),
        Elem::I16(v) => f64::from(v),
        Elem::U16(v) => f64::from(v),
        Elem::I32(v) => f64::from(v),
        Elem::I64(v) => v as f64,
        Elem::F32(v) => sse::widen(v),
    }
}

/// A voxel of a volume with voxels inside its extent and frames, read as a double (the samplers'
/// fast path: the byte place §6's row tables give, element `x` of row `y` of slice `z + f depth`,
/// none of FS-289's or FS-290's places); `None` for any other read (and for a 64-bit volume under
/// `reproduce`, FS-298).
#[inline]
fn inside_voxel(m: &Mri, x: i32, y: i32, z: i32, f: i32, reproduce: bool) -> Option<f64> {
    inside_elem(m, x, y, z, f, reproduce).map(elem_f64)
}

/// The element [`inside_voxel`] reads.
#[inline]
pub(crate) fn inside_elem(
    m: &Mri,
    x: i32,
    y: i32,
    z: i32,
    f: i32,
    reproduce: bool,
) -> Option<Elem> {
    let v = m.voxels.as_deref()?;
    let g = &m.geom;
    if x < 0 || y < 0 || z < 0 || f < 0 || x >= g.width || y >= g.height || z >= g.depth {
        return None;
    }
    if f >= m.nframes || m.kind == types::TENSOR || (m.kind == types::LONG && reproduce) {
        return None;
    }
    let n = super::arith::elem_size(m.kind)? as usize;
    if n as u64 != m.bytes_per_vox {
        return None;
    }
    let i = ((i64::from(f) * i64::from(g.depth) + i64::from(z)) * i64::from(g.height)
        + i64::from(y))
        * i64::from(g.width)
        + i64::from(x);
    let o = usize::try_from(i).ok()? * n;
    let b = v.get(o..o + n)?;
    Some(Elem::decode(m.kind, b))
}

/// C's `ceil` / `floor` of a double as an `int` (x86-64's conversion).
fn ceil_i(c: f64) -> i32 {
    cvtt(c.ceil())
}

fn floor_i(c: f64) -> i32 {
    cvtt(c.floor())
}

impl Process {
    /// One voxel of frame `f` read through the row tables as a double (§18: the trilinear, cubic
    /// and sinc samplers), of a type [`interpolated`] says they read. A 64-bit volume: its value
    /// by default; FreeSurfer's 32-bit integer at the voxel's place with `reproduce` (FS-298).
    #[allow(clippy::too_many_arguments)]
    fn sampler_read(
        &mut self,
        m: &Mri,
        x: i32,
        y: i32,
        z: i32,
        f: i32,
        what: &str,
    ) -> Result<f64, Stop> {
        let reproduce = self.mode().reproduce();
        if m.kind == types::LONG && !self.noted(&FS_298) {
            let d = if reproduce {
                "a 64-bit volume read as 32-bit integers at the voxels' places"
            } else {
                "a 64-bit volume sampled at its voxels' values"
            };
            self.note(&FS_298, format!("{what}: {d}"));
        }
        if let Some(v) = inside_voxel(m, x, y, z, f, reproduce) {
            return Ok(v);
        }
        self.direct_needs_voxels(m, what)?;
        let (x, y, z, f) = (i64::from(x), i64::from(y), i64::from(z), i64::from(f));
        if m.kind == types::LONG && reproduce {
            let place = self.place_of(m, x, y, z, f, 4);
            let b = self.direct_read8(m, place, 4, what);
            return Ok(f64::from(i32::from_le_bytes([b[0], b[1], b[2], b[3]])));
        }
        let n = super::arith::elem_size(m.kind).expect("a sampled type");
        let place = self.place_of(m, x, y, z, f, n);
        let b = self.direct_read8(m, place, n as usize, what);
        Ok(elem_f64(Elem::decode(m.kind, &b)))
    }

    /// The nearest sample of frame `f` (§15, §18): `outside_val` for a point unambiguously
    /// outside; else the voxel at the rounded and clamped coordinates as a float. `name` is the
    /// function the error names (`MRIsampleVolumeType`, `MRIsampleVolumeFrameType`).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn nearest_frame(
        &mut self,
        m: &Mri,
        x: f64,
        y: f64,
        z: f64,
        f: i32,
        name: &str,
        pval: &mut f64,
    ) -> Result<i32, Stop> {
        if self.mri_index_not_in_volume(m, x, y, z) == 1 {
            *pval = m.outside_val;
            return Ok(codes::NONE);
        }
        let e = [m.geom.width, m.geom.height, m.geom.depth];
        let p = [x, y, z];
        let mut q = [0i32; 3];
        for k in 0..3 {
            q[k] = self.nint(p[k]).max(0).min(e[k] - 1);
        }
        if !nearest_type(m.kind) {
            *pval = 0.0;
            self.error_printf(
                codes::UNSUPPORTED,
                format!("{name}: unsupported volume type {}", m.kind).as_bytes(),
            );
            return Ok(codes::UNSUPPORTED);
        }
        let v = self.nearest_voxel(m, q, f, name);
        *pval = sse::widen(v);
        Ok(codes::NONE)
    }

    /// The nearest sampler's voxel read (§15): a float; a volume without voxels, or a frame past
    /// the volume's, reads memory FreeSurfer does not own (FS-290: 0).
    pub(crate) fn nearest_voxel(&mut self, m: &Mri, q: [i32; 3], f: i32, name: &str) -> f32 {
        if let Some(e) = inside_elem(m, q[0], q[1], q[2], f, false) {
            return e.to_f32();
        }
        if !m.has_voxels() {
            self.note(
                &FS_290,
                format!("{name}: a volume without voxels read as 0 (FreeSurfer reads memory it does not own)"),
            );
            return 0.0;
        }
        let n = m.bytes_per_vox;
        let place = self.place_of(
            m,
            i64::from(q[0]),
            i64::from(q[1]),
            i64::from(q[2]),
            i64::from(f),
            n,
        );
        let b = self.direct_read8(m, place, n as usize, name);
        Elem::decode(m.kind, &b).to_f32()
    }

    /// `MRIsampleVolume(mri, x, y, z, &val)` (§18): trilinear, frame 0.
    pub fn mri_sample_volume(
        &mut self,
        m: &Mri,
        x: f64,
        y: f64,
        z: f64,
        pval: &mut f64,
    ) -> Result<i32, Stop> {
        self.trilinear(m, x, y, z, None, pval)
    }

    /// `MRIsampleVolumeFrame(mri, x, y, z, frame, &val)` (§18): trilinear on a frame; a frame at
    /// or past the volume's gives `outside_val` (after the whole-coordinate shortcut).
    #[allow(clippy::too_many_arguments)]
    pub fn mri_sample_volume_frame(
        &mut self,
        m: &Mri,
        x: f64,
        y: f64,
        z: f64,
        frame: i32,
        pval: &mut f64,
    ) -> Result<i32, Stop> {
        self.trilinear(m, x, y, z, Some(frame), pval)
    }

    #[allow(clippy::too_many_arguments)]
    fn trilinear(
        &mut self,
        m: &Mri,
        x: f64,
        y: f64,
        z: f64,
        frame: Option<i32>,
        pval: &mut f64,
    ) -> Result<i32, Stop> {
        let (name, near) = match frame {
            None => ("MRIsampleVolume", "MRIsampleVolumeType"),
            Some(_) => ("MRIsampleVolumeFrame", "MRIsampleVolumeFrameType"),
        };
        let f = frame.unwrap_or(0);
        if whole3(x, y, z) {
            return self.nearest_frame(m, x, y, z, f, near, pval);
        }
        if f >= m.nframes && frame.is_some() {
            *pval = m.outside_val;
            return Ok(codes::NONE);
        }
        if self.mri_index_not_in_volume(m, x, y, z) == 1 {
            *pval = m.outside_val;
            return Ok(codes::NONE);
        }
        if !interpolated(m.kind) {
            self.error_printf(
                codes::UNSUPPORTED,
                format!("{name}: unsupported type {}", m.kind).as_bytes(),
            );
            return Ok(codes::UNSUPPORTED);
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
            md[k] = sse::sub64(c, f64::from(lo[k] as f32));
            pd[k] = sse::sub64(1.0, md[k]);
        }
        let mut t = [0f64; 8];
        let mut n = 0;
        for cx in 0..2 {
            for cy in 0..2 {
                for cz in 0..2 {
                    let px = if cx == 0 { lo[0] } else { hi[0] };
                    let py = if cy == 0 { lo[1] } else { hi[1] };
                    let pz = if cz == 0 { lo[2] } else { hi[2] };
                    let wx = if cx == 0 { pd[0] } else { md[0] };
                    let wy = if cy == 0 { pd[1] } else { md[1] };
                    let wz = if cz == 0 { pd[2] } else { md[2] };
                    let v = self.sampler_read(m, px, py, pz, f, name)?;
                    t[n] = sse::mul64(sse::mul64(sse::mul64(wx, wy), wz), v);
                    n += 1;
                }
            }
        }
        // the official build's operand order (the left operand's NaN kept)
        let mut s = sse::add64(t[0], t[1]);
        s = sse::add64(s, t[2]);
        s = sse::add64(t[3], s);
        s = sse::add64(s, t[4]);
        s = sse::add64(t[5], s);
        s = sse::add64(t[6], s);
        s = sse::add64(s, t[7]);
        *pval = s;
        Ok(codes::NONE)
    }

    /// `MRIsampleVolumeType(mri, x, y, z, &val, type)` (§18): 1 trilinear, 3 cubic, 2 sinc with
    /// the half-width 5, every other code the nearest sample of frame 0.
    pub fn mri_sample_volume_type(
        &mut self,
        m: &Mri,
        x: f64,
        y: f64,
        z: f64,
        pval: &mut f64,
        kind: i32,
    ) -> Result<i32, Stop> {
        match kind {
            sample_type::TRILINEAR => self.mri_sample_volume(m, x, y, z, pval),
            sample_type::CUBIC => self.mri_cubic_sample_volume(m, x, y, z, pval),
            sample_type::SINC => self.mri_sinc_sample_volume(m, x, y, z, 5, pval),
            _ => self.nearest_frame(m, x, y, z, 0, "MRIsampleVolumeType", pval),
        }
    }

    /// `MRIsampleVolumeFrameType(mri, x, y, z, frame, type, &val)` (§18): whole coordinates the
    /// nearest sample of the frame; else 1 trilinear on the frame, 5 and 2 an error
    /// (`ERROR_UNSUPPORTED`, the value unchanged), every other code the nearest sample.
    #[allow(clippy::too_many_arguments)]
    pub fn mri_sample_volume_frame_type(
        &mut self,
        m: &Mri,
        x: f64,
        y: f64,
        z: f64,
        frame: i32,
        kind: i32,
        pval: &mut f64,
    ) -> Result<i32, Stop> {
        let name = "MRIsampleVolumeFrameType";
        if whole3(x, y, z) {
            return self.nearest_frame(m, x, y, z, frame, name, pval);
        }
        match kind {
            sample_type::TRILINEAR => self.mri_sample_volume_frame(m, x, y, z, frame, pval),
            sample_type::CUBIC_BSPLINE => {
                self.error_printf(
                    codes::UNSUPPORTED,
                    b"MRIsampleVolumeFrameType(5): First create coeff image and then use it to sample (see mriBSpline).",
                );
                Ok(codes::UNSUPPORTED)
            }
            sample_type::SINC => {
                self.error_printf(
                    codes::UNSUPPORTED,
                    b"MRIsampleVolumeFrameType(2): unsupported interpolation type",
                );
                Ok(codes::UNSUPPORTED)
            }
            _ => self.nearest_frame(m, x, y, z, frame, name, pval),
        }
    }

    /// `MRIcubicCoeff(x, i)` (§18): the cubic weight of window point `i` (0 to 3) for the
    /// fraction `x`; another `i` gives an error and -3 (observed: the probe's `coef`; the spec
    /// says 3).
    pub fn mri_cubic_coeff(&mut self, x: f64, i: i32) -> f64 {
        use sse::{add64, mul64, sub64};
        match i {
            0 => mul64(sub64(mul64(sub64(2.0, x), x), 1.0), x),
            1 => add64(mul64(mul64(sub64(mul64(3.0, x), 5.0), x), x), 2.0),
            2 => mul64(add64(mul64(sub64(4.0, mul64(3.0, x)), x), 1.0), x),
            3 => mul64(mul64(sub64(x, 1.0), x), x),
            _ => {
                self.error_printf(
                    codes::BAD_PARM,
                    b"MRIcubicCoeff: called wrong by MRIcubicSampleVolume!",
                );
                -3.0
            }
        }
    }

    /// `MRIcubicSampleVolume(mri, x, y, z, &val)` (§18): the cubic sum of the window's points
    /// inside the volume, divided by 8 (FS-299: `reproduce` divides by the number of points).
    pub fn mri_cubic_sample_volume(
        &mut self,
        m: &Mri,
        x: f64,
        y: f64,
        z: f64,
        pval: &mut f64,
    ) -> Result<i32, Stop> {
        self.cubic(m, x, y, z, None, pval)
    }

    /// `MRIcubicSampleVolumeFrame(mri, x, y, z, frame, &val)` (§18): the same on a frame (not
    /// checked), every point of the 4 x 4 x 4 window summed (those outside as 0 by default; FS-300:
    /// `reproduce` takes the double of all bits set the official build reads there), the sum times
    /// 0.125.
    #[allow(clippy::too_many_arguments)]
    pub fn mri_cubic_sample_volume_frame(
        &mut self,
        m: &Mri,
        x: f64,
        y: f64,
        z: f64,
        frame: i32,
        pval: &mut f64,
    ) -> Result<i32, Stop> {
        self.cubic(m, x, y, z, Some(frame), pval)
    }

    #[allow(clippy::too_many_arguments)]
    fn cubic(
        &mut self,
        m: &Mri,
        x: f64,
        y: f64,
        z: f64,
        frame: Option<i32>,
        pval: &mut f64,
    ) -> Result<i32, Stop> {
        let near = if frame.is_some() {
            "MRIsampleVolumeFrameType"
        } else {
            "MRIsampleVolumeType"
        };
        let name = "MRIcubicSampleVolume";
        let f = frame.unwrap_or(0);
        if whole3(x, y, z) {
            return self.nearest_frame(m, x, y, z, f, near, pval);
        }
        if self.mri_index_not_in_volume(m, x, y, z) == 1 {
            *pval = m.outside_val;
            return Ok(codes::NONE);
        }
        let e = [m.geom.width, m.geom.height, m.geom.depth];
        let mut low = [0i32; 3];
        let mut w = [[0f64; 4]; 3];
        for (k, c) in [x, y, z].into_iter().enumerate() {
            let mut c = c;
            if c > f64::from(e[k] - 1) {
                c = f64::from(e[k] - 1);
            }
            if c < 0.0 {
                c = 0.0;
            }
            let mut l = floor_i(c);
            let fr = if l >= e[k] - 1 {
                l -= 1;
                1.0
            } else {
                sse::sub64(c, f64::from(l))
            };
            low[k] = l;
            for (i, wi) in w[k].iter_mut().enumerate() {
                *wi = self.mri_cubic_coeff(fr, i as i32);
            }
        }
        // the window's points on each axis: all four for the frame variant, else those inside
        let range = |k: usize| -> (i32, i32) {
            if frame.is_some() {
                (0, 4)
            } else {
                ((1 - low[k]).max(0), (e[k] + 1 - low[k]).min(4))
            }
        };
        let (rx, ry, rz) = (range(0), range(1), range(2));
        let mut val = 0.0f64;
        let mut count = 0i32;
        let mut outside = false;
        for k in rz.0..rz.1 {
            for j in ry.0..ry.1 {
                for i in rx.0..rx.1 {
                    let (px, py, pz) = (low[0] - 1 + i, low[1] - 1 + j, low[2] - 1 + k);
                    let inside = (0..e[0]).contains(&px)
                        && (0..e[1]).contains(&py)
                        && (0..e[2]).contains(&pz);
                    let v = if inside {
                        if !interpolated(m.kind) {
                            self.error_printf(
                                codes::UNSUPPORTED,
                                format!("{name}: unsupported type {}", m.kind).as_bytes(),
                            );
                            return Ok(codes::UNSUPPORTED);
                        }
                        self.sampler_read(m, px, py, pz, f, name)?
                    } else {
                        // FS-300: 0 in both modes (FreeSurfer adds memory it never reads)
                        outside = true;
                        0.0
                    };
                    let term = sse::mul64(
                        sse::mul64(sse::mul64(w[0][i as usize], v), w[1][j as usize]),
                        w[2][k as usize],
                    );
                    val = sse::add64(term, val);
                    count += 1;
                }
            }
        }
        if frame.is_some() {
            if outside {
                self.note(
                    &FS_300,
                    "MRIcubicSampleVolumeFrame: the window's points outside the volume added as 0 (FreeSurfer adds memory it never reads)",
                );
            }
            *pval = sse::mul64(val, 0.125);
            return Ok(codes::NONE);
        }
        let d = if count != 8 {
            if self.noted(&FS_299) {
                if self.mode().reproduce() {
                    f64::from(count)
                } else {
                    8.0
                }
            } else if self.mode().reproduce() {
                self.note(
                    &FS_299,
                    format!("MRIcubicSampleVolume: the sum divided by the {count} points added"),
                );
                f64::from(count)
            } else {
                self.note(
                    &FS_299,
                    format!("MRIcubicSampleVolume: the sum divided by 8, its weights' sum (FreeSurfer divides by the {count} points added)"),
                );
                8.0
            }
        } else {
            8.0
        };
        *pval = sse::div64(val, d);
        Ok(codes::NONE)
    }

    /// `ham_sinc(x, fullwidth)` (§18): 1 within 1e-5 of 0, else the Hamming-windowed sinc
    /// (`sin`, `cos` correctly rounded by default, glibc 2.35's with `reproduce`).
    pub fn ham_sinc(&mut self, x: f64, fullwidth: f64) -> f64 {
        use sse::{add64, div64, mul64};
        if x.abs() < 1e-5 {
            return 1.0;
        }
        let px = mul64(std::f64::consts::PI, x);
        let s = self.c_trig("sin", px, math::sin, math::sin_glibc);
        let a = div64(mul64(2.0 * std::f64::consts::PI, x), fullwidth);
        let c = self.c_trig("cos", a, math::cos, math::cos_glibc);
        // the official build's operand order: the window's NaN kept where both are NaN
        mul64(add64(0.54, mul64(0.46, c)), div64(s, px))
    }

    /// The C library's `sin` or `cos` of a double (§18): an infinite argument gives x86-64's
    /// default NaN (sign set) in both modes; else [`Process::libm`].
    fn c_trig(&mut self, name: &str, x: f64, cr: fn(f64) -> f64, glibc: fn(f64) -> f64) -> f64 {
        if x.is_infinite() {
            return f64::from_bits(sse::NAN64);
        }
        self.libm(name, x, cr, glibc)
    }

    /// `MRIsincSampleVolume(mri, x, y, z, hw, &val)` (§18): the windowed sinc of half-width `hw`,
    /// frame 0.
    #[allow(clippy::too_many_arguments)]
    pub fn mri_sinc_sample_volume(
        &mut self,
        m: &Mri,
        x: f64,
        y: f64,
        z: f64,
        hw: i32,
        pval: &mut f64,
    ) -> Result<i32, Stop> {
        self.sinc(m, x, y, z, 0, hw, "MRIsincSampleVolume", pval)
    }

    /// `MRIsincSampleVolumeFrame(mri, x, y, z, frame, hw, &val)` (§18): the same on a frame (not
    /// checked).
    #[allow(clippy::too_many_arguments)]
    pub fn mri_sinc_sample_volume_frame(
        &mut self,
        m: &Mri,
        x: f64,
        y: f64,
        z: f64,
        frame: i32,
        hw: i32,
        pval: &mut f64,
    ) -> Result<i32, Stop> {
        self.sinc(m, x, y, z, frame, hw, "MRIsincSampleVolume", pval)
    }

    #[allow(clippy::too_many_arguments)]
    fn sinc(
        &mut self,
        m: &Mri,
        x: f64,
        y: f64,
        z: f64,
        f: i32,
        hw: i32,
        name: &str,
        pval: &mut f64,
    ) -> Result<i32, Stop> {
        if self.mri_index_not_in_volume(m, x, y, z) == 1 {
            *pval = m.outside_val;
            return Ok(codes::NONE);
        }
        let e = [m.geom.width, m.geom.height, m.geom.depth];
        let mut p = [x, y, z];
        for k in 0..3 {
            if p[k] >= f64::from(e[k]) {
                p[k] = f64::from(e[k] - 1);
            }
            if p[k] < 0.0 {
                p[k] = 0.0;
            }
        }
        if ceil_i(p[2]) == e[2] {
            *pval = 0.0;
            return Ok(codes::NONE);
        }
        let fullwidth = f64::from(hw.wrapping_mul(2));
        let mut first = [0i32; 3];
        let mut ws: [Vec<f64>; 3] = Default::default();
        let mut sums = [0f64; 3];
        for k in 0..3 {
            let lo = ceil_i(p[k]).wrapping_sub(hw).max(0);
            let hi = floor_i(p[k]).wrapping_add(hw).min(e[k] - 1);
            let n = i64::from(hi) - i64::from(lo);
            if n > 128 {
                // FS-302, both modes: the end of the program (FreeSurfer writes past its arrays)
                self.note(
                    &FS_302,
                    format!("{name}: a window of {n} voxels on an axis ends the program (FreeSurfer writes past its 128 weights)"),
                );
                self.error_exit(
                    codes::BAD_PARM,
                    format!("{name}: a half-width of {hw} needs more than 128 weights on an axis (FreeSurfer writes past its arrays here)\n")
                        .as_bytes(),
                )?;
                return Ok(codes::BAD_PARM);
            }
            first[k] = lo;
            let mut s = 0.0f64;
            for j in lo..hi {
                let w = self.ham_sinc(sse::sub64(p[k], f64::from(j)), fullwidth);
                s = sse::add64(w, s);
                ws[k].push(w);
            }
            sums[k] = s;
        }
        let mut sum_z = 0.0f64;
        for (kz, wz) in ws[2].iter().enumerate() {
            let mut sum_y = 0.0f64;
            for (ky, wy) in ws[1].iter().enumerate() {
                let mut sum_x = 0.0f64;
                for (kx, wx) in ws[0].iter().enumerate() {
                    let (px, py, pz) = (
                        first[0] + kx as i32,
                        first[1] + ky as i32,
                        first[2] + kz as i32,
                    );
                    if !interpolated(m.kind) {
                        self.error_printf(
                            codes::UNSUPPORTED,
                            format!("{name}: unsupported type {}", m.kind).as_bytes(),
                        );
                        return Ok(codes::UNSUPPORTED);
                    }
                    let v = self.sampler_read(m, px, py, pz, f, name)?;
                    sum_x = sse::add64(sum_x, sse::mul64(v, sse::div64(*wx, sums[0])));
                }
                sum_y = sse::add64(sum_y, sse::mul64(sum_x, sse::div64(*wy, sums[1])));
            }
            sum_z = sse::add64(sum_z, sse::mul64(sum_y, sse::div64(*wz, sums[2])));
        }
        // C's `if (v < 0) v = 0; if (v > hi) v = hi;` (`clamp` is the same: a NaN stays)
        *pval = match m.kind {
            types::UCHAR => sum_z.clamp(0.0, 255.0),
            types::SHORT => sum_z.clamp(0.0, 65535.0),
            _ => sum_z,
        };
        Ok(codes::NONE)
    }
}
