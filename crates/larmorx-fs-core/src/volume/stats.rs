// SPDX-License-Identifier: Apache-2.0
//! Centre of mass, principal components, the running average (spec §15, §15b) and the header
//! comparison, determinant, dump and derivative sign (§16).

use super::{Dst, VolOut, Vols};
use crate::clib::printf::sprintf_f64;
use crate::maths::Returned;
use crate::maths::matrix::{MATRIX_REAL, Matrix};
use crate::mri::Mri;
use crate::process::{Process, Stop, codes};
use crate::sse;

impl Process {
    /// `MRIindexNotInVolume(mri, c, r, s)` (§15): 0 inside, -1 when the coordinates rounded to
    /// the nearest integers (ties to even) are inside, else 1.
    pub fn mri_index_not_in_volume(&mut self, m: &Mri, c: f64, r: f64, s: f64) -> i32 {
        let e = [m.geom.width, m.geom.height, m.geom.depth];
        let p = [c, r, s];
        if (0..3).all(|k| p[k] >= 0.0 && p[k] <= f64::from(e[k] - 1)) {
            return 0;
        }
        let near = (0..3).all(|k| {
            let q = (p[k] as f32).round_ties_even();
            q >= 0.0 && q < e[k] as f32
        });
        if near { -1 } else { 1 }
    }

    /// `MRIsampleVolumeType(mri, x, y, z, &val, SAMPLE_NEAREST)` (§15): its value and the sample.
    pub fn mri_sample_nearest(
        &mut self,
        m: &Mri,
        x: f64,
        y: f64,
        z: f64,
    ) -> Result<(i32, f64), Stop> {
        let mut v = 0.0;
        let r = self.nearest_frame(m, x, y, z, 0, "MRIsampleVolumeType", &mut v)?;
        Ok((r, v))
    }

    /// `MRIcenterOfMass(mri, means, threshold)` (§15): the means of the voxels above the
    /// threshold, weighted by their values; zeros without weight. Returns `NO_ERROR`.
    pub fn mri_center_of_mass(&mut self, m: &Mri, threshold: u8) -> Result<(i32, [f64; 3]), Stop> {
        let (mut w, mut s) = (0.0f64, [0.0f64; 3]);
        let g = m.geom.clone();
        for z in 0..g.depth {
            for y in 0..g.height {
                for x in 0..g.width {
                    let (_, v) =
                        self.mri_sample_nearest(m, f64::from(x), f64::from(y), f64::from(z))?;
                    if v > f64::from(threshold) {
                        w = sse::add64(w, v);
                        for (k, c) in [x, y, z].into_iter().enumerate() {
                            s[k] = sse::add64(s[k], sse::mul64(sse::widen(c as f32), v));
                        }
                    }
                }
            }
        }
        let means = if w > 0.0 {
            [
                sse::div64(s[0], w),
                sse::div64(s[1], w),
                sse::div64(s[2], w),
            ]
        } else {
            [0.0; 3]
        };
        Ok((codes::NONE, means))
    }

    /// `MRIprincipleComponents(mri, evectors, evalues, means, threshold)` (§15): the weighted
    /// covariance of the voxels above the threshold and its eigensystem (vectors into
    /// `evectors`, values into `evalues` when it has one). Returns `NO_ERROR` and the means.
    pub fn mri_principle_components(
        &mut self,
        m: &Mri,
        evectors: &mut Matrix,
        evalues: &mut [f32; 3],
        threshold: u8,
    ) -> Result<(i32, [f64; 3]), Stop> {
        let g = m.geom.clone();
        let t = f32::from(threshold);
        let (mut w, mut s) = (0.0f64, [0.0f64; 3]);
        for z in 0..g.depth {
            for y in 0..g.height {
                for x in 0..g.width {
                    let v = self.mri_get_vox_val(m, x, y, z, 0, 0)?;
                    if v > t {
                        let v = sse::widen(v);
                        w = sse::add64(w, v);
                        for (k, c) in [x, y, z].into_iter().enumerate() {
                            s[k] = sse::add64(s[k], sse::mul64(sse::widen(c as f32), v));
                        }
                    }
                }
            }
        }
        let means = if w > 0.0 {
            [
                sse::div64(s[0], w),
                sse::div64(s[1], w),
                sse::div64(s[2], w),
            ]
        } else {
            [0.0; 3]
        };
        let mut cov = Matrix::zeros(3, 3, MATRIX_REAL);
        for z in 0..g.depth {
            for y in 0..g.height {
                for x in 0..g.width {
                    let v = self.mri_get_vox_val(m, x, y, z, 0, 0)?;
                    if v <= t || v.is_nan() {
                        continue;
                    }
                    let v = sse::widen(v);
                    let mut col = Matrix::zeros(3, 1, MATRIX_REAL);
                    for (k, c) in [x, y, z].into_iter().enumerate() {
                        let d = sse::mul64(sse::sub64(f64::from(c), means[k]), v);
                        col.set(k as i32 + 1, 1, sse::narrow(d));
                    }
                    let Returned::New(ct) = self.matrix_transpose(&col, None)? else {
                        continue;
                    };
                    let Returned::New(outer) = self.matrix_multiply(Some(&col), Some(&ct), None)?
                    else {
                        continue;
                    };
                    let prev = cov.clone();
                    if let Returned::New(sum) = self.matrix_add_sub(&prev, &outer, None, false)? {
                        cov = sum;
                    }
                }
            }
        }
        if w > 0.0 {
            let k = sse::narrow(sse::div64(1.0, w));
            let prev = cov.clone();
            if let Returned::New(c) = self.matrix_scalar_mul(&prev, k, None)? {
                cov = c;
            }
        }
        let (_, vals) = self.matrix_eigen_system(&cov, Some(evectors))?;
        if let Some(vals) = vals {
            for (o, v) in evalues.iter_mut().zip(vals) {
                *o = v;
            }
        }
        Ok((codes::NONE, means))
    }

    /// `MRIaverage(src, dof, dst)` (§15b): the running average of the source into the
    /// destination, whose `dof` is incremented.
    pub fn mri_average(&mut self, src: &mut Mri, dof: i32, dst: Dst) -> Result<VolOut, Stop> {
        let mut v = Vols::new(src, None, dst);
        if !v.has_dst() {
            let g = v.a().geom.clone();
            let mut d = self.mri_alloc_sequence(g.width, g.height, g.depth, v.a().kind, 1)?;
            self.copy_header_into(v.a(), &mut d)?;
            v.set_dst(d);
        }
        if self.mri_check_size(v.a(), Some(v.d()), 0, 0, 0) == 0 {
            self.error_printf(
                codes::BAD_PARM,
                b"MRIaverage: incompatible volume dimensions",
            );
            return Ok(VolOut::Nothing);
        }
        let n = dof.wrapping_add(1);
        let (g, nf) = (v.d().geom.clone(), v.d().nframes);
        for f in 0..nf {
            for z in 0..g.depth {
                for y in 0..g.height {
                    for x in 0..g.width {
                        let vs = self.frame_sample(v.a(), x, y, z, f);
                        let vd = self.frame_sample(v.d(), x, y, z, f);
                        let a =
                            sse::add64(sse::mul64(f64::from(dof), sse::widen(vd)), sse::widen(vs));
                        let o = sse::narrow(sse::div64(a, f64::from(n)));
                        self.mri_set_vox_val(v.d_mut(), x, y, z, f, o, 0)?;
                    }
                }
            }
        }
        let d = v.d_mut();
        d.dof = d.dof.wrapping_add(1);
        Ok(v.out())
    }

    /// `MRIsampleVolumeFrameType(mri, x, y, z, frame, SAMPLE_NEAREST)` at whole coordinates
    /// inside the volume (§15b): the voxel of that frame; 0 and the error for another type.
    fn frame_sample(&mut self, m: &Mri, x: i32, y: i32, z: i32, f: i32) -> f32 {
        if !super::sample::nearest_type(m.kind) {
            self.error_printf(
                codes::UNSUPPORTED,
                format!(
                    "MRIsampleVolumeFrameType: unsupported volume type {}",
                    m.kind
                )
                .as_bytes(),
            );
            return 0.0;
        }
        self.nearest_voxel(m, [x, y, z], f, "MRIsampleVolumeFrameType")
    }

    // ---- §16

    /// `MRIcompareHeaders(a, b)` (§16): 1 when either is absent or the headers differ, else 0.
    pub fn mri_compare_headers(&mut self, a: Option<&Mri>, b: Option<&Mri>) -> i32 {
        let (Some(a), Some(b)) = (a, b) else { return 1 };
        // differs by FLT_EPSILON or more, or a NaN difference
        let differ = |x: f32, y: f32| {
            sse::sub32(x, y).abs().partial_cmp(&f32::EPSILON) != Some(std::cmp::Ordering::Less)
        };
        let (g, h) = (&a.geom, &b.geom);
        let pairs = [
            (g.xsize, h.xsize),
            (g.ysize, h.ysize),
            (g.zsize, h.zsize),
            (a.fov, b.fov),
            (a.thick, b.thick),
            (a.ps, b.ps),
            (a.xstart, b.xstart),
            (a.xend, b.xend),
            (a.ystart, b.ystart),
            (a.yend, b.yend),
            (a.zstart, b.zstart),
            (a.zend, b.zend),
            (a.tr, b.tr),
            (a.te, b.te),
            (a.ti, b.ti),
            (g.x[0], h.x[0]),
            (g.x[1], h.x[1]),
            (g.x[2], h.x[2]),
            (g.y[0], h.y[0]),
            (g.y[1], h.y[1]),
            (g.y[2], h.y[2]),
            (g.z[0], h.z[0]),
            (g.z[1], h.z[1]),
            (g.z[2], h.z[2]),
            (g.c[0], h.c[0]),
            (g.c[1], h.c[1]),
            (g.c[2], h.c[2]),
        ];
        if pairs.iter().any(|&(x, y)| differ(x, y)) {
            return 1;
        }
        let flip = sse::sub64(a.flip_angle, b.flip_angle).abs();
        if flip.partial_cmp(&f64::from(f32::EPSILON)) != Some(std::cmp::Ordering::Less) {
            return 1;
        }
        #[allow(clippy::float_cmp)]
        let location_differs = a.location != b.location;
        if a.ptype != b.ptype || location_differs || g.ras_good_flag != h.ras_good_flag {
            return 1;
        }
        0
    }

    /// `MRIvolumeDeterminant(mri)` (§16): the determinant of the direction cosines' matrix.
    pub fn mri_volume_determinant(&mut self, m: &Mri) -> f32 {
        let g = &m.geom;
        let mut d = Matrix::zeros(4, 4, MATRIX_REAL);
        for (j, c) in [g.x, g.y, g.z].iter().enumerate() {
            for (r, &v) in c.iter().enumerate() {
                d.set(r as i32 + 1, j as i32 + 1, v);
            }
        }
        d.set(4, 4, 1.0);
        self.matrix_determinant(&d)
    }

    /// `MRIdump(mri, stdout)` (§16): the header's lines on standard output; returns `NO_ERROR`.
    pub fn mri_dump(&mut self, m: &Mri) -> i32 {
        let mut b = Vec::new();
        let name6 = |n: &str| {
            let t: String = n.chars().take(6).collect();
            format!("{t:>6}")
        };
        let line = |b: &mut Vec<u8>, n: &str, v: &[u8]| {
            b.extend_from_slice(name6(n).as_bytes());
            b.extend_from_slice(b" = ");
            b.extend_from_slice(v);
            b.push(b'\n');
        };
        let f = |x: f32| sprintf_f64(b"%f", sse::widen(x));
        let cstr = |s: &[u8]| s[..s.iter().position(|&c| c == 0).unwrap_or(s.len())].to_vec();
        let g = &m.geom;
        line(&mut b, "fname", &cstr(&g.fname));
        for (n, v) in [
            ("height", g.height),
            ("width", g.width),
            ("depth", g.depth),
            ("nframes", m.nframes),
            ("imnr0", m.imnr0),
            ("imnr1", m.imnr1),
            ("xnum", g.width),
            ("ynum", g.height),
        ] {
            line(&mut b, n, v.to_string().as_bytes());
        }
        for (n, v) in [
            ("fov", m.fov),
            ("thick", m.thick),
            ("xstart", m.xstart),
            ("xend", m.xend),
            ("ystart", m.ystart),
            ("yend", m.yend),
            ("zstart", m.zstart),
            ("zend", m.zend),
        ] {
            line(&mut b, n, &f(v));
        }
        line(&mut b, "type", m.kind.to_string().as_bytes());
        for (n, v) in [("xsize", g.xsize), ("ysize", g.ysize), ("zsize", g.zsize)] {
            line(&mut b, n, &f(v));
        }
        for (n, v) in [
            ("x ras", g.x),
            ("y ras", g.y),
            ("z ras", g.z),
            ("c ras", g.c),
        ] {
            let mut t = f(v[0]);
            for x in &v[1..] {
                t.push(b' ');
                t.extend_from_slice(&f(*x));
            }
            line(&mut b, n, &t);
        }
        let det = self.mri_volume_determinant(m);
        b.extend_from_slice(b"det(xyz_ras) = ");
        b.extend_from_slice(&f(det));
        b.push(b'\n');
        b.extend_from_slice(format!("ras_good_flag = {}\n", g.ras_good_flag).as_bytes());
        b.extend_from_slice(format!("brightness = {}\n", m.brightness).as_bytes());
        for (n, s) in [
            ("subject_name", &m.subject_name),
            ("path_to_t1", &m.path_to_t1),
            ("fname_format", &m.fname_format),
        ] {
            b.extend_from_slice(n.as_bytes());
            b.extend_from_slice(b" = ");
            b.extend_from_slice(&cstr(s));
            b.push(b'\n');
        }
        self.put_out(&b);
        if let Some(r) = g.register_mat.clone() {
            self.put_out(b"register_mat = \n");
            self.matrix_print(Some(false), Some(&r), 5);
        }
        codes::NONE
    }

    /// `MRIsetSampleVolumeDerivativeSign(s)` (§16): the process's derivative sign set, its line on
    /// standard output; returns `s`.
    pub fn mri_set_sample_volume_derivative_sign(&mut self, s: f64) -> f64 {
        self.derivative_sign = s;
        let mut b = b"Settting MRIsampleVolumeDerivativeSign to ".to_vec();
        b.extend_from_slice(&sprintf_f64(b"%g", s));
        b.push(b'\n');
        self.put_out(&b);
        s
    }
}
