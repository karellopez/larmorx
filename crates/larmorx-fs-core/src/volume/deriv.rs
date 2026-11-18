// SPDX-License-Identifier: Apache-2.0
//! Directional derivatives, gradients and neighbour counts (spec §28):
//! `MRIsampleVolumeDerivative(Scale)`, `MRIsampleVolumeGradient(Frame)` and the eight neighbour
//! counts; FS-322 to FS-324.

use larmorx_core::defects::{FS_322, FS_323, FS_324};
use larmorx_core::math;

use super::sample::FLT_EPSILON;
use crate::mri::Mri;
use crate::process::{Process, Stop, codes};
use crate::sse;

/// C's `MAX(a, b)` of doubles, `a >= b ? a : b`.
fn cmax(a: f64, b: f64) -> f64 {
    if a >= b { a } else { b }
}

/// The neighbour counts (§28).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Neighbours {
    /// `MRIneighborsOn(mri, x, y, z, min)`: face neighbours `>= min`.
    On(i32),
    /// `MRIneighborsOn3x3(mri, x, y, z, min)`: the 26 neighbours `> min`.
    On3x3(i32),
    /// `MRIneighborsInWindow(mri, x, y, z, wsize, val)`: the window's voxels `== val`.
    InWindow(i32, i32),
    /// `MRIneighbors(mri, x, y, z, val)`: face neighbours rounding to `val`.
    Value(i32),
    /// `MRIneighborsInRange(mri, x, y, z, frame, low, high)`: face neighbours within the range.
    InRange(i32, f32, f32),
    /// `MRIneighbors3x3(mri, x, y, z, val)`: the 26 neighbours rounding to `val`.
    Value3x3(i32),
    /// `MRIneighborsOff(mri, x, y, z, min)`: face neighbours whose byte is `< min`.
    Off(i32),
    /// `MRIneighborsOff3x3(mri, x, y, z, min)`: the 26 neighbours whose byte is `< min`.
    Off3x3(i32),
}

impl Neighbours {
    /// The function's name (its messages).
    pub fn name(self) -> &'static str {
        match self {
            Neighbours::On(_) => "MRIneighborsOn",
            Neighbours::On3x3(_) => "MRIneighborsOn3x3",
            Neighbours::InWindow(..) => "MRIneighborsInWindow",
            Neighbours::Value(_) => "MRIneighbors",
            Neighbours::InRange(..) => "MRIneighborsInRange",
            Neighbours::Value3x3(_) => "MRIneighbors3x3",
            Neighbours::Off(_) => "MRIneighborsOff",
            Neighbours::Off3x3(_) => "MRIneighborsOff3x3",
        }
    }
}

/// The point clamped to the extent on each axis (§28: `x >= width` gives `width - 1`, then
/// `x < 0` gives 0; a NaN kept).
fn clamp_point(m: &Mri, p: [f64; 3]) -> [f64; 3] {
    let e = [m.geom.width, m.geom.height, m.geom.depth];
    let mut q = p;
    for k in 0..3 {
        if q[k] >= f64::from(e[k]) {
            q[k] = f64::from(e[k] - 1);
        }
        if q[k] < 0.0 {
            q[k] = 0.0;
        }
    }
    q
}

/// The point plus `d` times the direction, each coordinate `x + (d dx)` in `double`.
fn along(p: [f64; 3], d: f64, dir: [f64; 3], plus: bool) -> [f64; 3] {
    let mut q = [0.0; 3];
    for k in 0..3 {
        let s = sse::mul64(d, dir[k]);
        q[k] = if plus {
            sse::add64(p[k], s)
        } else {
            sse::sub64(p[k], s)
        };
    }
    q
}

impl Process {
    /// One sample of the derivatives and gradients (§18's `MRIsampleVolume`, or
    /// `MRIsampleVolumeFrame`) into `val`; a refused sample leaves it, and where it was never
    /// written it is 0 (FS-322, noted).
    fn deriv_sample(
        &mut self,
        m: &Mri,
        p: [f64; 3],
        frame: Option<i32>,
        val: &mut f64,
        written: &mut bool,
        what: &str,
    ) -> Result<(), Stop> {
        let w = self.sample_kept(m, p[0], p[1], p[2], frame, val)?;
        if !w && !*written {
            self.note(
                &FS_322,
                format!("{what}: a refused sample's value never written taken as 0 (FreeSurfer's is memory contents)"),
            );
        }
        *written |= w;
        Ok(())
    }

    /// `MRIsampleVolumeDerivative(mri, x, y, z, dx, dy, dz, &mag)` (§28): the samples at 0.5,
    /// 1, 1.5 and 2 along the direction and against it from the clamped point; `mag` the
    /// difference of their means over twice the mean distance, times the derivative sign (§16).
    /// Returns 0.
    #[allow(clippy::too_many_arguments)]
    pub fn mri_sample_volume_derivative(
        &mut self,
        m: &Mri,
        x: f64,
        y: f64,
        z: f64,
        dx: f64,
        dy: f64,
        dz: f64,
        mag: &mut f64,
    ) -> Result<i32, Stop> {
        const WHAT: &str = "MRIsampleVolumeDerivative";
        let p = clamp_point(m, [x, y, z]);
        let dir = [dx, dy, dz];
        let (mut val, mut written) = (0.0, false);
        let (mut vp, mut vm, mut len) = (0.0f64, 0.0f64, 0.0f64);
        for d in [0.5, 1.0, 1.5, 2.0] {
            len = sse::add64(len, d);
            self.deriv_sample(
                m,
                along(p, d, dir, true),
                None,
                &mut val,
                &mut written,
                WHAT,
            )?;
            vp = sse::add64(vp, val);
            self.deriv_sample(
                m,
                along(p, d, dir, false),
                None,
                &mut val,
                &mut written,
                WHAT,
            )?;
            vm = sse::add64(vm, val);
        }
        vm = sse::div64(vm, 4.0);
        vp = sse::div64(vp, 4.0);
        len = sse::div64(len, 4.0);
        *mag = sse::mul64(
            sse::div64(sse::sub64(vp, vm), sse::mul64(2.0, len)),
            self.derivative_sign,
        );
        Ok(0)
    }

    /// `MRIsampleVolumeDerivativeScale(mri, x, y, z, dx, dy, dz, &mag, sigma)` (§28): the same at
    /// the distances `step, 2 step, ...` up to `MAX(2 sigma, step)` (`step = MAX(0.25, sigma /
    /// 5)`), weighted by a Gaussian of `sigma` (1 for a scale within `FLT_EPSILON` of 0). FS-324:
    /// a scale whose double is `+infinity` (FreeSurfer never returns) ends the program. Returns 0.
    #[allow(clippy::too_many_arguments)]
    pub fn mri_sample_volume_derivative_scale(
        &mut self,
        m: &Mri,
        x: f64,
        y: f64,
        z: f64,
        dx: f64,
        dy: f64,
        dz: f64,
        mag: &mut f64,
        sigma: f64,
    ) -> Result<i32, Stop> {
        const WHAT: &str = "MRIsampleVolumeDerivativeScale";
        if sse::mul64(2.0, sigma) == f64::INFINITY {
            self.note(
                &FS_324,
                "MRIsampleVolumeDerivativeScale: a scale twice which is infinite refused (FreeSurfer never returns)",
            );
            self.error_exit(
                codes::BAD_PARM,
                format!("{WHAT}: the scale {sigma} is too large (FreeSurfer never returns)\n")
                    .as_bytes(),
            )?;
            return Ok(0);
        }
        let p = clamp_point(m, [x, y, z]);
        let dir = [dx, dy, dz];
        let step = cmax(0.25, sse::div64(sigma, 5.0));
        let dmax = cmax(sse::mul64(2.0, sigma), step);
        let flat = sigma.abs() < FLT_EPSILON;
        let denom = sse::mul64(sse::mul64(2.0, sigma), sigma);
        let (mut val, mut written) = (0.0, false);
        let (mut vp, mut vm, mut len, mut ktotal) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        let mut d = step;
        while d <= dmax {
            let k = if flat {
                1.0
            } else {
                let a = sse::div64(-sse::mul64(d, d), denom);
                self.libm("exp", a, math::exp, math::exp_glibc)
            };
            ktotal = sse::add64(ktotal, k);
            len = sse::add64(len, d);
            self.deriv_sample(
                m,
                along(p, d, dir, true),
                None,
                &mut val,
                &mut written,
                WHAT,
            )?;
            vp = sse::add64(vp, sse::mul64(k, val));
            self.deriv_sample(
                m,
                along(p, d, dir, false),
                None,
                &mut val,
                &mut written,
                WHAT,
            )?;
            vm = sse::add64(vm, sse::mul64(k, val));
            if step < FLT_EPSILON {
                break;
            }
            d = sse::add64(d, step);
        }
        vm = sse::div64(vm, ktotal);
        vp = sse::div64(vp, ktotal);
        len = sse::div64(len, ktotal);
        *mag = sse::mul64(
            sse::div64(sse::sub64(vp, vm), sse::mul64(2.0, len)),
            self.derivative_sign,
        );
        Ok(0)
    }

    /// `MRIsampleVolumeGradient(mri, x, y, z, &dx, &dy, &dz)` (§28): central differences of the
    /// samples one voxel along each axis (the point itself where that neighbour is not plainly
    /// inside), over twice the voxel size. Returns 0.
    #[allow(clippy::too_many_arguments)]
    pub fn mri_sample_volume_gradient(
        &mut self,
        m: &Mri,
        x: f64,
        y: f64,
        z: f64,
        dx: &mut f64,
        dy: &mut f64,
        dz: &mut f64,
    ) -> Result<i32, Stop> {
        self.gradient(m, [x, y, z], None, [dx, dy, dz])
    }

    /// `MRIsampleVolumeGradientFrame(mri, x, y, z, &dx, &dy, &dz, frame)` (§28): the same with
    /// `MRIsampleVolumeFrame` at the six neighbours always. Returns 0.
    #[allow(clippy::too_many_arguments)]
    pub fn mri_sample_volume_gradient_frame(
        &mut self,
        m: &Mri,
        x: f64,
        y: f64,
        z: f64,
        dx: &mut f64,
        dy: &mut f64,
        dz: &mut f64,
        frame: i32,
    ) -> Result<i32, Stop> {
        self.gradient(m, [x, y, z], Some(frame), [dx, dy, dz])
    }

    fn gradient(
        &mut self,
        m: &Mri,
        p: [f64; 3],
        frame: Option<i32>,
        out: [&mut f64; 3],
    ) -> Result<i32, Stop> {
        let what = if frame.is_some() {
            "MRIsampleVolumeGradientFrame"
        } else {
            "MRIsampleVolumeGradient"
        };
        let mut v = [0.0f64; 6];
        for (i, slot) in v.iter_mut().enumerate() {
            let (k, plus) = (i / 2, i % 2 == 0);
            let mut q = p;
            q[k] = if plus {
                sse::add64(p[k], 1.0)
            } else {
                sse::sub64(p[k], 1.0)
            };
            if frame.is_none() && self.mri_index_not_in_volume(m, q[0], q[1], q[2]) != 0 {
                q = p;
            }
            let mut written = false;
            self.deriv_sample(m, q, frame, slot, &mut written, what)?;
        }
        let size = [m.geom.xsize, m.geom.ysize, m.geom.zsize];
        for (k, o) in out.into_iter().enumerate() {
            *o = sse::div64(
                sse::sub64(v[2 * k], v[2 * k + 1]),
                sse::mul64(2.0, sse::widen(size[k])),
            );
        }
        Ok(0)
    }

    /// One index through the volume's boundary index table of an axis of extent `e`
    /// (`specs/freesurfer-mri.md` §4 step 7); FS-323 (both modes): an index outside the table
    /// ends the program.
    fn table_index(&mut self, e: i32, i: i32, name: &str) -> Result<i32, Stop> {
        let (e64, i64v) = (i64::from(e), i64::from(i));
        if i64v < -500 || i64v > e64 + 499 {
            self.note(
                &FS_323,
                format!("{name}: a voxel index past the volume's index tables refused (FreeSurfer reads memory it does not own)"),
            );
            self.error_exit(
                codes::BAD_PARM,
                format!("{name}: the voxel index {i} lies past the volume's index tables (FreeSurfer reads memory it does not own)\n").as_bytes(),
            )?;
            return Ok(0);
        }
        Ok(if i <= 0 {
            0
        } else if i >= e {
            e - 1
        } else {
            i
        })
    }

    /// The neighbour counts (§28): the number of neighbours of voxel (x, y, z) that the count
    /// takes, frame 0 unless named.
    pub fn mri_neighbours(
        &mut self,
        m: &Mri,
        x: i32,
        y: i32,
        z: i32,
        count: Neighbours,
    ) -> Result<i32, Stop> {
        let name = count.name();
        let e = [m.geom.width, m.geom.height, m.geom.depth];
        let p = [x, y, z];
        // the voxels the count reads, in its order, every table index checked first
        let mut at: Vec<[i32; 3]> = Vec::with_capacity(27);
        match count {
            Neighbours::On(_) | Neighbours::Value(_) | Neighbours::InRange(..) => {
                self.direct_needs_voxels(m, name)?;
                for (k, d) in [(0, -1), (0, 1), (1, 1), (1, -1), (2, 1), (2, -1)] {
                    let mut q = p;
                    q[k] = self.table_index(e[k], p[k].wrapping_add(d), name)?;
                    at.push(q);
                }
            }
            Neighbours::Off(_) => {
                self.direct_needs_voxels(m, name)?;
                for (k, d) in [(0, -1), (0, 1), (1, 1), (1, -1), (2, 1), (2, -1)] {
                    let mut q = p;
                    q[k] = p[k].wrapping_add(d);
                    at.push(q);
                }
            }
            Neighbours::On3x3(_) | Neighbours::Value3x3(_) | Neighbours::Off3x3(_) => {
                self.direct_needs_voxels(m, name)?;
                self.window(&mut at, e, p, 1, name)?;
            }
            Neighbours::InWindow(wsize, _) => {
                self.direct_needs_voxels(m, name)?;
                self.window(&mut at, e, p, (wsize - 1) / 2, name)?;
            }
        }
        let mut n = 0i32;
        for q in at {
            let hit = match count {
                Neighbours::Off(min) | Neighbours::Off3x3(min) => {
                    let place =
                        self.row_place(m, i64::from(q[0]), i64::from(q[1]), i64::from(q[2]), 0, 1);
                    let b = self.direct_read8(m, place, 1, name)[0];
                    i32::from(b) < min
                }
                Neighbours::InRange(frame, lo, hi) => {
                    let v = self.mri_get_vox_val(m, q[0], q[1], q[2], frame, 0)?;
                    v >= lo && v <= hi
                }
                _ => {
                    let v = self.mri_get_vox_val(m, q[0], q[1], q[2], 0, 0)?;
                    match count {
                        Neighbours::On(min) => v >= min as f32,
                        Neighbours::On3x3(min) => v > min as f32,
                        Neighbours::InWindow(_, val) => v == val as f32,
                        Neighbours::Value(val) | Neighbours::Value3x3(val) => {
                            self.nint(sse::widen(v)) == val
                        }
                        _ => false,
                    }
                }
            };
            if hit {
                n += 1;
            }
        }
        Ok(n)
    }

    /// The window of half-width `half` around `p` without `p` itself (z outermost, then y, x),
    /// every coordinate through its table (§28).
    fn window(
        &mut self,
        at: &mut Vec<[i32; 3]>,
        e: [i32; 3],
        p: [i32; 3],
        half: i32,
        name: &str,
    ) -> Result<(), Stop> {
        if half < 0 {
            return Ok(());
        }
        for dz in -half..=half {
            for dy in -half..=half {
                for dx in -half..=half {
                    if dx == 0 && dy == 0 && dz == 0 {
                        continue;
                    }
                    let zi = self.table_index(e[2], p[2].wrapping_add(dz), name)?;
                    let yi = self.table_index(e[1], p[1].wrapping_add(dy), name)?;
                    let xi = self.table_index(e[0], p[0].wrapping_add(dx), name)?;
                    at.push([xi, yi, zi]);
                }
            }
        }
        Ok(())
    }
}
