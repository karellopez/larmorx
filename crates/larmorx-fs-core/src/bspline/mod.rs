// SPDX-License-Identifier: Apache-2.0
//! FreeSurfer 8.2.0's B-splines (`fs.bspline`), a clean room of `specs/freesurfer-bspline.md`:
//! the B-spline record (§2), its coefficients (§3: the recursive line filter of Thévenaz, Blu and
//! Unser's "Interpolation Revisited" along x, y and z), sampling (§4, `sample`), the linear
//! transforms of a volume through it (§5, `transform`) and the factor-of-two pyramids of the
//! centred cubic spline (§6, `pyramid`); FS-962 to FS-964.
//!
//! Every line of the filter is independent of the others, so the passes give the same result
//! whatever the order of the lines; arithmetic is x86-64's SSE2 in `double` with its NaN rules
//! made explicit (`crate::sse`), and the C library's `log` and `pow` come from the program's mode
//! (correctly rounded by default, glibc's under `reproduce`).

pub mod pyramid;
pub mod sample;
pub mod transform;

use crate::mri::{Mri, types};
use crate::process::{Process, Stop, codes};
use crate::sse;

/// A B-spline (§2): its degree, its `float` coefficients, the source's type and whether the
/// source had a negative voxel.
#[derive(Clone, Debug)]
pub struct BSpline {
    pub degree: i32,
    pub coeff: Option<Mri>,
    pub srctype: i32,
    pub srcneg: i32,
}

/// `DBL_EPSILON`.
const DBL_EPSILON: f64 = f64::EPSILON;

/// The poles of a degree (§3.2's table), none for a degree outside 2 to 9.
pub fn poles(degree: i32) -> Vec<f64> {
    match degree {
        2 => vec![8f64.sqrt() - 3.0],
        3 => vec![3f64.sqrt() - 2.0],
        4 => vec![
            ((664.0 - 438976f64.sqrt()).sqrt() + 304f64.sqrt()) - 19.0,
            ((664.0 + 438976f64.sqrt()).sqrt() - 304f64.sqrt()) - 19.0,
        ],
        5 => vec![
            ((135.0 / 2.0 - (17745.0f64 / 4.0).sqrt()).sqrt() + (105.0f64 / 4.0).sqrt())
                - 13.0 / 2.0,
            ((135.0 / 2.0 + (17745.0f64 / 4.0).sqrt()).sqrt() - (105.0f64 / 4.0).sqrt())
                - 13.0 / 2.0,
        ],
        6 => vec![
            -0.488_294_589_303_044_755_130_118_038_883_789_062_112_279_161_239_38,
            -0.081_679_271_076_237_512_597_937_765_737_059_080_653_379_610_398_148,
            -0.001_414_151_808_325_817_751_087_243_976_558_592_527_864_169_055_346_7,
        ],
        7 => vec![
            -0.535_280_430_796_438_165_542_403_781_681_646_071_833_923_152_342_69,
            -0.122_554_615_192_326_690_515_272_264_359_357_343_605_486_549_427_30,
            -0.009_148_694_809_608_276_928_593_021_651_647_853_415_692_563_954_599_4,
        ],
        8 => vec![
            -0.574_686_909_248_765_430_530_139_304_128_745_424_290_661_578_041_25,
            -0.163_035_269_297_280_935_240_551_896_860_737_052_234_768_145_508_30,
            -0.023_632_294_694_844_850_023_403_919_296_361_320_612_665_920_854_629,
            -0.000_153_821_310_641_690_911_739_352_530_184_021_607_629_640_540_700_43,
        ],
        9 => vec![
            -0.607_997_389_168_625_779_007_720_823_954_289_769_439_634_718_539_91,
            -0.201_750_520_193_153_238_796_064_685_055_970_434_680_898_865_757_47,
            -0.043_222_608_540_481_752_133_321_142_979_429_688_265_852_380_231_497,
            -0.002_121_306_903_180_818_420_304_896_557_848_623_422_054_856_098_862_4,
        ],
        _ => Vec::new(),
    }
}

/// A line filter's constants for one degree and one line length: the gain, and per pole its
/// value, its horizon and (for the full start) `pow(z, n - 1)`.
pub(crate) struct LineFilter {
    gain: f64,
    poles: Vec<(f64, i64, f64)>,
    n: usize,
}

impl Process {
    /// The line filter of `degree` for lines of `n` samples (§3.2): the horizons from the C
    /// library's `log` and the full start's powers from its `pow`, in this program's mode.
    pub(crate) fn line_filter(&mut self, degree: i32, n: usize) -> LineFilter {
        let zs = poles(degree);
        let mut gain = 1.0f64;
        for &z in &zs {
            gain = (gain * (1.0 - z)) * (1.0 - 1.0 / z);
        }
        let le = self.libm(
            "log",
            DBL_EPSILON,
            larmorx_core::math::log,
            larmorx_core::math::log_glibc,
        );
        let mut ps = Vec::with_capacity(zs.len());
        for &z in &zs {
            let lz = self.libm(
                "log",
                z.abs(),
                larmorx_core::math::log,
                larmorx_core::math::log_glibc,
            );
            let h = (le / lz).ceil() as i64;
            let p = if h < n as i64 || n < 2 {
                0.0
            } else {
                self.libm_pow(z, (n - 1) as f64)
            };
            ps.push((z, h, p));
        }
        LineFilter { gain, poles: ps, n }
    }
}

impl LineFilter {
    /// Filters one line of `n` samples in place (§3.2); a line of one sample is left as it is.
    pub(crate) fn apply(&self, c: &mut [f64]) {
        let n = self.n;
        debug_assert_eq!(c.len(), n);
        if n < 2 {
            return;
        }
        if c.iter().all(|v| v.is_finite()) {
            // every intermediate is then a finite double (float inputs, constants below 1 in
            // magnitude): the plain operations give SSE's bits
            self.apply_finite(c);
            return;
        }
        for v in c.iter_mut() {
            *v = sse::mul64(*v, self.gain);
        }
        for &(z, h, z2n0) in &self.poles {
            let s = if h < n as i64 {
                let mut s = c[0];
                let mut zn = z;
                for &ci in &c[1..h as usize] {
                    s = sse::add64(s, sse::mul64(ci, zn));
                    zn *= z;
                }
                s
            } else {
                let mut zn = z;
                let iz = 1.0 / z;
                let mut z2n = z2n0;
                let mut s = sse::add64(sse::mul64(c[n - 1], z2n), c[0]);
                z2n *= z2n * iz;
                for &ci in &c[1..n - 1] {
                    s = sse::add64(s, sse::mul64(zn + z2n, ci));
                    zn *= z;
                    z2n *= iz;
                }
                sse::div64(s, 1.0 - zn * zn)
            };
            c[0] = s;
            for i in 1..n {
                c[i] = sse::add64(sse::mul64(c[i - 1], z), c[i]);
            }
            c[n - 1] = sse::mul64(
                z / (z * z - 1.0),
                sse::add64(sse::mul64(c[n - 2], z), c[n - 1]),
            );
            for i in (0..n - 1).rev() {
                c[i] = sse::mul64(sse::sub64(c[i + 1], c[i]), z);
            }
        }
    }

    /// [`LineFilter::apply`] of a line of finite samples, with the plain operations.
    fn apply_finite(&self, c: &mut [f64]) {
        let n = self.n;
        for v in c.iter_mut() {
            *v *= self.gain;
        }
        for &(z, h, z2n0) in &self.poles {
            let s = if h < n as i64 {
                let mut s = c[0];
                let mut zn = z;
                for &ci in &c[1..h as usize] {
                    s += ci * zn;
                    zn *= z;
                }
                s
            } else {
                let mut zn = z;
                let iz = 1.0 / z;
                let mut z2n = z2n0;
                let mut s = c[n - 1] * z2n + c[0];
                z2n *= z2n * iz;
                for &ci in &c[1..n - 1] {
                    s += (zn + z2n) * ci;
                    zn *= z;
                    z2n *= iz;
                }
                s / (1.0 - zn * zn)
            };
            c[0] = s;
            for i in 1..n {
                c[i] = c[i - 1] * z + c[i];
            }
            c[n - 1] = (z / (z * z - 1.0)) * (c[n - 2] * z + c[n - 1]);
            for i in (0..n - 1).rev() {
                c[i] = (c[i + 1] - c[i]) * z;
            }
        }
    }
}

/// Which axis a line runs along.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Axis {
    X,
    Y,
    Z,
}

/// The element index of voxel (x, y, z, f) of a volume of the extent `(w, h, d)` in its buffer
/// (rows of x, then y, slices, frames).
#[inline]
pub(crate) fn elem(w: usize, h: usize, d: usize, x: usize, y: usize, z: usize, f: usize) -> usize {
    ((f * d + z) * h + y) * w + x
}

/// Whether a volume holds its voxels as plain `float` elements (the coefficients' fast path).
pub(crate) fn plain_float(m: &Mri) -> bool {
    let g = &m.geom;
    m.kind == types::FLOAT
        && m.bytes_per_vox == 4
        && m.voxels.as_ref().is_some_and(|v| {
            let n = (g.width.max(0) as usize)
                * (g.height.max(0) as usize)
                * (g.depth.max(0) as usize)
                * (m.nframes.max(0) as usize);
            v.len() >= 4 * n
        })
}

/// The float at element `i` of a plain float volume.
#[inline]
pub(crate) fn fget(v: &[u8], i: usize) -> f32 {
    f32::from_le_bytes([v[4 * i], v[4 * i + 1], v[4 * i + 2], v[4 * i + 3]])
}

/// Sets the float at element `i` of a plain float volume.
#[inline]
pub(crate) fn fset(v: &mut [u8], i: usize, x: f32) {
    v[4 * i..4 * i + 4].copy_from_slice(&x.to_le_bytes());
}

impl Process {
    /// `MRIallocBSpline(width, height, depth, nframes)` (§2): `float` coefficients of that
    /// extent, degree -1, source type -1, no negative voxel.
    pub fn mri_alloc_bspline(
        &mut self,
        width: i32,
        height: i32,
        depth: i32,
        nframes: i32,
    ) -> Result<BSpline, Stop> {
        let c = self.mri_alloc_sequence(width, height, depth, types::FLOAT, nframes)?;
        Ok(BSpline {
            degree: -1,
            coeff: Some(c),
            srctype: -1,
            srcneg: 0,
        })
    }

    /// `MRIfreeBSpline(&b)` (§2): `ERROR_BADPARM` after its error without a B-spline; else the
    /// coefficients freed (`MRIfree`'s own error when there are none), the B-spline gone, 0.
    pub fn mri_free_bspline(&mut self, b: &mut Option<BSpline>) -> i32 {
        match b.take() {
            None => {
                self.error_printf(codes::BAD_PARM, b"MRIfreeBSpline: null pointer\n");
                codes::BAD_PARM
            }
            Some(mut s) => {
                self.mri_free(&mut s.coeff);
                codes::NONE
            }
        }
    }

    /// "Ends the program (message, status 1)" (§1): the message and a newline on standard
    /// output, exit status 1.
    pub(crate) fn bspline_end(&mut self, message: &str) -> Stop {
        self.put_out(message.as_bytes());
        self.put_out(b"\n");
        self.flush();
        Stop::Exit(1)
    }

    /// New coefficients for `src` (§3.1 step 2): a `float` volume of its extent and frames with
    /// its header and its outside value.
    fn bspline_coefficients_for(&mut self, src: &Mri) -> Result<Mri, Stop> {
        let g = &src.geom;
        let mut c =
            self.mri_alloc_sequence(g.width, g.height, g.depth, types::FLOAT, src.nframes)?;
        self.mri_copy_header(src, Some(&mut c))?;
        c.outside_val = src.outside_val;
        Ok(c)
    }

    /// `MRItoBSpline(src, b, degree)` (§3): the coefficients of `src` for `degree` into the given
    /// B-spline (its coefficients as they are, new ones when it has none) or a new one.
    pub fn mri_to_bspline(
        &mut self,
        src: &Mri,
        b: Option<BSpline>,
        degree: i32,
    ) -> Result<BSpline, Stop> {
        self.put_out(format!("MRItoBSpline degree {degree}\n").as_bytes());
        let g = &src.geom;
        let mut b = match b {
            None => {
                let mut nb = self.mri_alloc_bspline(g.width, g.height, g.depth, src.nframes)?;
                self.mri_copy_header(src, nb.coeff.as_mut())?;
                if let Some(c) = nb.coeff.as_mut() {
                    c.outside_val = src.outside_val;
                }
                nb
            }
            Some(mut b) => {
                if b.coeff.is_none() {
                    b.coeff = Some(self.bspline_coefficients_for(src)?);
                }
                b
            }
        };
        b.degree = degree;
        b.srctype = src.kind;
        b.srcneg = 0;
        let mut c = b.coeff.take().expect("coefficients");
        if c.kind != types::FLOAT {
            return Err(
                self.bspline_end("ERROR MRItoBSpline: BSpline Coeff MRI needs to be float!")
            );
        }
        let cg = &c.geom;
        if cg.width != g.width
            || cg.height != g.height
            || cg.depth != g.depth
            || c.nframes != src.nframes
        {
            return Err(self.bspline_end(
                "ERROR MRItoBSpline: BSPline Coeff MRI dimensions must be same as Source MRI!",
            ));
        }
        if !(2..=9).contains(&degree) {
            return Err(self.bspline_end("Invalid spline degree"));
        }
        let (w, h, d, nf) = (
            g.width.max(0) as usize,
            g.height.max(0) as usize,
            g.depth.max(0) as usize,
            src.nframes.max(0) as usize,
        );
        // pass x: the source's lines, the negative test, the filter, the store
        let fx = self.line_filter(degree, w);
        let mut line = vec![0f64; w];
        let mut neg = false;
        for z in 0..d {
            for y in 0..h {
                for f in 0..nf {
                    self.read_src_line(src, y, z, f, &mut line)?;
                    if !neg && line.iter().any(|&v| v < 0.0) {
                        neg = true;
                    }
                    fx.apply(&mut line);
                    self.store_line(&mut c, Axis::X, [0, y, z], f, &line)?;
                }
            }
        }
        if neg {
            b.srcneg = 1;
        }
        // passes y and z on the coefficients
        for (axis, n) in [(Axis::Y, h), (Axis::Z, d)] {
            if n <= 1 {
                continue;
            }
            let flt = self.line_filter(degree, n);
            self.filter_coefficients(&mut c, axis, &flt)?;
        }
        b.coeff = Some(c);
        Ok(b)
    }

    /// Row `(y, z, f)` of a source through `MRIgetVoxVal`, widened (plain float and byte
    /// volumes read directly: the same values).
    fn read_src_line(
        &mut self,
        src: &Mri,
        y: usize,
        z: usize,
        f: usize,
        out: &mut [f64],
    ) -> Result<(), Stop> {
        let g = &src.geom;
        let (w, h, d) = (g.width as usize, g.height as usize, g.depth as usize);
        if plain_float(src) {
            let v = src.voxels.as_deref().expect("voxels");
            let base = elem(w, h, d, 0, y, z, f);
            for (x, o) in out.iter_mut().enumerate() {
                *o = sse::widen(fget(v, base + x));
            }
            return Ok(());
        }
        for (x, o) in out.iter_mut().enumerate() {
            let v = self.mri_get_vox_val(src, x as i32, y as i32, z as i32, f as i32, 0)?;
            *o = sse::widen(v);
        }
        Ok(())
    }

    /// Passes y and z (§3.1 steps 7, 8): every line along `axis` of the coefficients read back,
    /// filtered and stored.
    fn filter_coefficients(
        &mut self,
        c: &mut Mri,
        axis: Axis,
        flt: &LineFilter,
    ) -> Result<(), Stop> {
        let g = &c.geom;
        let (w, h, d, nf) = (
            g.width as usize,
            g.height as usize,
            g.depth as usize,
            c.nframes as usize,
        );
        let n = if axis == Axis::Y { h } else { d };
        let mut line = vec![0f64; n];
        if plain_float(c) {
            let v = c.voxels.as_deref_mut().expect("voxels");
            let (outer, stride) = match axis {
                Axis::Y => (d, w),
                _ => (h, w * h),
            };
            for f in 0..nf {
                for o in 0..outer {
                    for x in 0..w {
                        let base = match axis {
                            Axis::Y => elem(w, h, d, x, 0, o, f),
                            _ => elem(w, h, d, x, o, 0, f),
                        };
                        for (k, l) in line.iter_mut().enumerate() {
                            *l = sse::widen(fget(v, base + k * stride));
                        }
                        flt.apply(&mut line);
                        for (k, l) in line.iter().enumerate() {
                            fset(v, base + k * stride, sse::narrow(*l));
                        }
                    }
                }
            }
            return Ok(());
        }
        let outer = if axis == Axis::Y { d } else { h };
        for o in 0..outer {
            for x in 0..w {
                for f in 0..nf {
                    for (k, l) in line.iter_mut().enumerate() {
                        let (y, z) = if axis == Axis::Y { (k, o) } else { (o, k) };
                        let v =
                            self.mri_get_vox_val(c, x as i32, y as i32, z as i32, f as i32, 0)?;
                        *l = sse::widen(v);
                    }
                    flt.apply(&mut line);
                    let at = if axis == Axis::Y {
                        [x, 0, o]
                    } else {
                        [x, o, 0]
                    };
                    self.store_line(c, axis, at, f, &line)?;
                }
            }
        }
        Ok(())
    }

    /// Stores a line (§3.4) into the target along `axis` from `at` (the axis's own index 0),
    /// frame `f`: the target's extent along the axis of samples, a byte target clipped to 0 and
    /// 255 and rounded with `nint`, any other `(float)` of the sample, through `MRIsetVoxVal`.
    /// A line index outside the target ends the program (status 1).
    pub(crate) fn store_line(
        &mut self,
        t: &mut Mri,
        axis: Axis,
        at: [usize; 3],
        f: usize,
        line: &[f64],
    ) -> Result<(), Stop> {
        let g = &t.geom;
        let ext = [g.width as usize, g.height as usize, g.depth as usize];
        let checks: [(usize, &str); 2] = match axis {
            Axis::X => [(1, "Y limit error"), (2, "Z limit error")],
            Axis::Y => [(0, "X limit error"), (2, "Z limit error")],
            Axis::Z => [(0, "X limit error"), (1, "Y limit error")],
        };
        for (k, msg) in checks {
            if at[k] >= ext[k] {
                return Err(self.bspline_end(msg));
            }
        }
        if f >= t.nframes.max(0) as usize {
            return Err(self.bspline_end("Frames limit error"));
        }
        let k_axis = match axis {
            Axis::X => 0,
            Axis::Y => 1,
            Axis::Z => 2,
        };
        let n = ext[k_axis];
        let byte = t.kind == types::UCHAR;
        if !byte && plain_float(t) {
            let (w, h, d) = (ext[0], ext[1], ext[2]);
            let stride = [1, w, w * h][k_axis];
            let base = elem(w, h, d, at[0], at[1], at[2], f);
            let v = t.voxels.as_deref_mut().expect("voxels");
            for (k, l) in line.iter().take(n).enumerate() {
                fset(v, base + k * stride, sse::narrow(*l));
            }
            return Ok(());
        }
        for (k, &l) in line.iter().take(n).enumerate() {
            let v = if byte {
                if l > 255.0 {
                    255.0
                } else if l < 0.0 {
                    0.0
                } else {
                    self.nint(l) as f32
                }
            } else {
                sse::narrow(l)
            };
            let mut p = at;
            p[k_axis] = k;
            self.mri_set_vox_val(t, p[0] as i32, p[1] as i32, p[2] as i32, f as i32, v, 0)?;
        }
        Ok(())
    }
}
