// SPDX-License-Identifier: Apache-2.0
//! Sampling a B-spline (spec §4): the indices with mirror boundaries, the weights of each degree
//! (§4.3, evaluated as written), the sums; FS-964.

use larmorx_core::defects::FS_964;

use super::{BSpline, elem, fget, plain_float};
use crate::mri::Mri;
use crate::process::{Process, Stop, codes};
use crate::sse;

/// The arithmetic of the sums: plain IEEE operations, or SSE's NaN rules made explicit.
#[derive(Clone, Copy)]
struct Ops {
    add: fn(f64, f64) -> f64,
    mul: fn(f64, f64) -> f64,
}

const PLAIN: Ops = Ops {
    add: |a, b| a + b,
    mul: |a, b| a * b,
};

const SSE: Ops = Ops {
    add: sse::add64,
    mul: sse::mul64,
};

/// The indices and weights of one axis: `n` of them (1 on an axis of one voxel).
#[derive(Clone, Copy)]
pub(crate) struct AxisWeights {
    pub idx: [i32; 10],
    pub w: [f64; 10],
    pub n: usize,
}

/// The weights of `t` against the indices `i` for a degree of 2 to 9 (§4.3).
#[allow(clippy::many_single_char_names)]
fn weights(degree: i32, t: f64, i: &[i32; 10]) -> [f64; 10] {
    let mut w = [0f64; 10];
    let at = |k: usize| t - f64::from(i[k]);
    match degree {
        2 => {
            let s = at(1);
            w[1] = 3.0 / 4.0 - s * s;
            w[2] = (1.0 / 2.0) * ((s - w[1]) + 1.0);
            w[0] = (1.0 - w[1]) - w[2];
        }
        3 => {
            let s = at(1);
            w[3] = (((1.0 / 6.0) * s) * s) * s;
            w[0] = ((1.0 / 6.0) + ((1.0 / 2.0) * s) * (s - 1.0)) - w[3];
            w[2] = (s + w[0]) - 2.0 * w[3];
            w[1] = ((1.0 - w[0]) - w[2]) - w[3];
        }
        4 => {
            let s = at(2);
            let s2 = s * s;
            let u = (1.0 / 6.0) * s2;
            let mut w0 = 1.0 / 2.0 - s;
            w0 *= w0;
            w0 *= (1.0 / 24.0) * w0;
            let t0 = s * (u - 11.0 / 24.0);
            let t1 = 19.0 / 96.0 + s2 * (1.0 / 4.0 - u);
            w[0] = w0;
            w[1] = t1 + t0;
            w[3] = t1 - t0;
            w[4] = (w[0] + t0) + (1.0 / 2.0) * s;
            w[2] = (((1.0 - w[0]) - w[1]) - w[3]) - w[4];
        }
        5 => {
            let mut s = at(2);
            let mut s2 = s * s;
            w[5] = (((1.0 / 120.0) * s) * s2) * s2;
            s2 -= s;
            let s4 = s2 * s2;
            s -= 1.0 / 2.0;
            let u = s2 * (s2 - 3.0);
            w[0] = (1.0 / 24.0) * ((1.0 / 5.0 + s2) + s4) - w[5];
            let t0 = (1.0 / 24.0) * (s2 * (s2 - 5.0) + 46.0 / 5.0);
            let t1 = ((-1.0 / 12.0) * s) * (u + 4.0);
            w[2] = t0 + t1;
            w[3] = t0 - t1;
            let t0 = (1.0 / 16.0) * (9.0 / 5.0 - u);
            let t1 = ((1.0 / 24.0) * s) * ((s4 - s2) - 5.0);
            w[1] = t0 + t1;
            w[4] = t0 - t1;
        }
        6 => {
            let s = at(3);
            let mut w0 = 1.0 / 2.0 - s;
            w0 *= w0 * w0;
            w0 *= w0 / 720.0;
            w[0] = w0;
            w[1] = (361.0 / 192.0
                - s * (59.0 / 8.0
                    + s * (-185.0 / 16.0
                        + s * (25.0 / 3.0 + (s * (-5.0 / 2.0 + s)) * (1.0 / 2.0 + s)))))
                / 120.0;
            w[2] = (10543.0 / 960.0
                + s * (-289.0 / 16.0
                    + s * (79.0 / 16.0 + s * (43.0 / 6.0 + s * (-17.0 / 4.0 + s * (-1.0 + s))))))
                / 48.0;
            let s2 = s * s;
            w[3] = (5887.0 / 320.0 - s2 * (231.0 / 16.0 - s2 * (21.0 / 4.0 - s2))) / 36.0;
            w[4] = (10543.0 / 960.0
                + s * (289.0 / 16.0
                    + s * (79.0 / 16.0 + s * (-43.0 / 6.0 + s * (-17.0 / 4.0 + s * (1.0 + s))))))
                / 48.0;
            let mut w6 = 1.0 / 2.0 + s;
            w6 *= w6 * w6;
            w6 *= w6 / 720.0;
            w[6] = w6;
            w[5] = (((((1.0 - w[0]) - w[1]) - w[2]) - w[3]) - w[4]) - w[6];
        }
        7 => {
            let s = at(3);
            let mut w0 = 1.0 - s;
            w0 *= w0;
            w0 *= w0 * w0;
            w0 *= (1.0 - s) / 5040.0;
            w[0] = w0;
            let s2 = s * s;
            w[1] = (120.0 / 7.0
                + s * (-56.0 + s * (72.0 + s * (-40.0 + s2 * (12.0 + s * (-6.0 + s))))))
                / 720.0;
            w[2] = (397.0 / 7.0
                - s * (245.0 / 3.0
                    + s * (-15.0 + s * (-95.0 / 3.0 + s * (15.0 + s * (5.0 + s * (-5.0 + s)))))))
                / 240.0;
            w[3] = (2416.0 / 35.0 + s2 * (-48.0 + s2 * (16.0 + s2 * (-4.0 + s)))) / 144.0;
            w[4] = (1191.0 / 35.0
                - s * (-49.0 + s * (-9.0 + s * (19.0 + (s * (-3.0 + s)) * (-3.0 + s2)))))
                / 144.0;
            w[5] = (40.0 / 7.0
                + s * (56.0 / 3.0 + s * (24.0 + s * (40.0 / 3.0 + s2 * (-4.0 + s * (-2.0 + s))))))
                / 240.0;
            let mut w7 = s2;
            w7 *= w7 * w7;
            w7 *= s / 5040.0;
            w[7] = w7;
            w[6] = ((((((1.0 - w[0]) - w[1]) - w[2]) - w[3]) - w[4]) - w[5]) - w[7];
        }
        8 => {
            let s = at(4);
            let mut w0 = 1.0 / 2.0 - s;
            w0 *= w0;
            w0 *= w0;
            w0 *= w0 / 40320.0;
            w[0] = w0;
            let s2 = s * s;
            w[1] = ((39.0 / 16.0 - s * (6.0 + s * (-9.0 / 2.0 + s2)))
                * (21.0 / 16.0 + s * (-15.0 / 4.0 + s * (9.0 / 2.0 + s * (-3.0 + s)))))
                / 5040.0;
            w[2] = (82903.0 / 1792.0
                + s * (-4177.0 / 32.0
                    + s * (2275.0 / 16.0
                        + s * (-487.0 / 8.0
                            + s * (-85.0 / 8.0
                                + s * (41.0 / 2.0 + s * (-5.0 + s * (-2.0 + s))))))))
                / 1440.0;
            w[3] = (310661.0 / 1792.0
                - s * (14219.0 / 64.0
                    + s * (-199.0 / 8.0
                        + s * (-1327.0 / 16.0
                            + s * (245.0 / 8.0
                                + s * (53.0 / 4.0 + s * (-8.0 + s * (-1.0 + s))))))))
                / 720.0;
            w[4] = (2337507.0 / 8960.0
                + s2 * (-2601.0 / 16.0 + s2 * (387.0 / 8.0 + s2 * (-9.0 + s2))))
                / 576.0;
            w[5] = (310661.0 / 1792.0
                - s * (-14219.0 / 64.0
                    + s * (-199.0 / 8.0
                        + s * (1327.0 / 16.0
                            + s * (245.0 / 8.0
                                + s * (-53.0 / 4.0 + s * (-8.0 + s * (1.0 + s))))))))
                / 720.0;
            w[7] = ((39.0 / 16.0 - s * (-6.0 + s * (-9.0 / 2.0 + s2)))
                * (21.0 / 16.0 + s * (15.0 / 4.0 + s * (9.0 / 2.0 + s * (3.0 + s)))))
                / 5040.0;
            let mut w8 = 1.0 / 2.0 + s;
            w8 *= w8;
            w8 *= w8;
            w8 *= w8 / 40320.0;
            w[8] = w8;
            w[6] = (((((((1.0 - w[0]) - w[1]) - w[2]) - w[3]) - w[4]) - w[5]) - w[7]) - w[8];
        }
        9 => {
            let s = at(4);
            let mut w0 = 1.0 - s;
            w0 *= w0;
            w0 *= w0;
            w0 *= (w0 * (1.0 - s)) / 362880.0;
            w[0] = w0;
            w[1] = (502.0 / 9.0
                + s * (-246.0
                    + s * (472.0
                        + s * (-504.0
                            + s * (308.0
                                + s * (-84.0
                                    + s * (-56.0 / 3.0 + s * (24.0 + s * (-8.0 + s)))))))))
                / 40320.0;
            w[2] = (3652.0 / 9.0
                - s * (2023.0 / 2.0
                    + s * (-952.0
                        + s * (938.0 / 3.0
                            + s * (112.0
                                + s * (-119.0
                                    + s * (56.0 / 3.0 + s * (14.0 + s * (-7.0 + s)))))))))
                / 10080.0;
            w[3] = (44117.0 / 42.0
                + s * (-2427.0 / 2.0
                    + s * (66.0
                        + s * (434.0
                            + s * (-129.0
                                + s * (-69.0 + s * (34.0 + s * (6.0 + s * (-6.0 + s)))))))))
                / 4320.0;
            let s2 = s * s;
            w[4] = (78095.0 / 63.0
                - s2 * (700.0 + s2 * (-190.0 + s2 * (100.0 / 3.0 + s2 * (-5.0 + s)))))
                / 2880.0;
            w[5] = (44117.0 / 63.0
                + s * (809.0
                    + s * (44.0
                        + s * (-868.0 / 3.0
                            + s * (-86.0
                                + s * (46.0 + s * (68.0 / 3.0 + s * (-4.0 + s * (-4.0 + s)))))))))
                / 2880.0;
            w[6] = (3652.0 / 21.0
                - s * (-867.0 / 2.0
                    + s * (-408.0
                        + s * (-134.0
                            + s * (48.0
                                + s * (51.0 + ((s * (-4.0 + s)) * (-1.0 + s)) * (2.0 + s)))))))
                / 4320.0;
            w[7] = (251.0 / 18.0
                + s * (123.0 / 2.0
                    + s * (118.0
                        + s * (126.0
                            + s * (77.0
                                + s * (21.0 + s * (-14.0 / 3.0 + s * (-6.0 + s * (-2.0 + s)))))))))
                / 10080.0;
            let mut w9 = s2 * s2;
            w9 *= (w9 * s) / 362880.0;
            w[9] = w9;
            w[8] =
                ((((((((1.0 - w[0]) - w[1]) - w[2]) - w[3]) - w[4]) - w[5]) - w[6]) - w[7]) - w[9];
        }
        _ => {}
    }
    w
}

/// The mirror boundary of an index on an axis of `n > 1` voxels (§4.1 step 3).
#[inline]
fn mirror(i: i32, n: i32) -> i32 {
    let n2 = 2 * n - 2;
    let j = if i < 0 {
        -i - n2 * ((-i) / n2)
    } else {
        i - n2 * (i / n2)
    };
    if j >= n { n2 - j } else { j }
}

/// The indices (mirrored) and weights of the coordinate `t` on an axis of `n` voxels for a
/// degree of 2 to 9; the single index 0 and weight 1 on an axis of one voxel.
pub(crate) fn axis_weights(degree: i32, t: f64, n: i32) -> AxisWeights {
    let mut a = AxisWeights {
        idx: [0; 10],
        w: [0.0; 10],
        n: 1,
    };
    if n <= 1 {
        a.w[0] = 1.0;
        return a;
    }
    let d = degree as usize;
    let half = i64::from(degree / 2);
    let i0 = if degree % 2 == 1 {
        (t.floor() - half as f64) as i64
    } else {
        (t + 0.5).floor() as i64 - half
    };
    for k in 0..=d {
        a.idx[k] = (i0 + k as i64) as i32;
    }
    a.w = weights(degree, t, &a.idx);
    for k in 0..=d {
        a.idx[k] = mirror(a.idx[k], n);
    }
    a.n = d + 1;
    a
}

/// What a sampler does for a B-spline's degree on these coefficients (§4.1's last paragraph).
enum DegreeCase {
    /// Sampled (degrees 2 to 9, or 0 and 1 on a volume of one voxel).
    Sample,
    /// FS-964: refused with its error.
    Fs964,
    /// FS-964 under `reproduce`, FreeSurfer's end: `Invalid spline degree`.
    Fs964Ends,
    /// `Invalid spline degree`: the end of the program.
    Invalid,
}

/// FS-964's case under `reproduce` where FreeSurfer's end is deterministic: a degree above 9 on
/// coefficients with an axis longer than one voxel writes its indices past their arrays, then
/// its weights end the program (`Invalid spline degree`, status 1; observed: `vbsrules`,
/// degrees 10 and 11 on 3 x 3 x 3 coefficients, the same in every run).
fn degree_case(degree: i32, c: &Mri, reproduce: bool) -> DegreeCase {
    let g = &c.geom;
    let one = g.width <= 1 && g.height <= 1 && g.depth <= 1;
    if degree > 9 && !one && reproduce {
        DegreeCase::Fs964Ends
    } else if degree > 9 || (degree < 0 && one) {
        DegreeCase::Fs964
    } else if (2..=9).contains(&degree) || one {
        DegreeCase::Sample
    } else {
        DegreeCase::Invalid
    }
}

impl Process {
    /// The weights of the three axes for a sampler, or its refusal (FS-964: `Err(None)`, after the
    /// error) or the end of the program.
    fn sampler_weights(
        &mut self,
        b: &BSpline,
        c: &Mri,
        p: [f64; 3],
        function: &str,
    ) -> Result<Option<[AxisWeights; 3]>, Stop> {
        let reproduce = self.mode().reproduce();
        match degree_case(b.degree, c, reproduce) {
            DegreeCase::Fs964Ends => {
                self.note(
                    &FS_964,
                    format!(
                        "{function}: a B-spline of degree {} ended the program as FreeSurfer's (after its indices were written past their arrays)",
                        b.degree
                    ),
                );
                Err(self.bspline_end("Invalid spline degree"))
            }
            DegreeCase::Fs964 => {
                self.note(
                    &FS_964,
                    format!(
                        "{function}: a B-spline of degree {} refused (FreeSurfer reads or writes memory it does not own)",
                        b.degree
                    ),
                );
                self.error_printf(
                    codes::BAD_PARM,
                    format!(
                        "{function}: a B-spline of degree {} (FreeSurfer reads or writes memory it does not own here)",
                        b.degree
                    )
                    .as_bytes(),
                );
                Ok(None)
            }
            DegreeCase::Invalid => Err(self.bspline_end("Invalid spline degree")),
            DegreeCase::Sample => {
                let g = &c.geom;
                Ok(Some([
                    axis_weights(b.degree, p[0], g.width),
                    axis_weights(b.degree, p[1], g.height),
                    axis_weights(b.degree, p[2], g.depth),
                ]))
            }
        }
    }

    /// The sum of §4.1 step 4 on frame `f`, clipped as step 5: with plain operations, and again
    /// with SSE's NaN rules made explicit when a NaN reaches the sum (without one both give the
    /// same bits).
    fn bspline_sum(
        &mut self,
        b: &BSpline,
        c: &Mri,
        a: &[AxisWeights; 3],
        f: i32,
    ) -> Result<f64, Stop> {
        let mut sum = self.bspline_sum_with(c, a, f, PLAIN)?;
        if sum.is_nan() {
            sum = self.bspline_sum_with(c, a, f, SSE)?;
        }
        if b.srcneg == 0 && sum < 0.0 {
            sum = 0.0;
        }
        Ok(sum)
    }

    fn bspline_sum_with(
        &mut self,
        c: &Mri,
        a: &[AxisWeights; 3],
        f: i32,
        o: Ops,
    ) -> Result<f64, Stop> {
        let [ax, ay, az] = a;
        let g = &c.geom;
        let fast = plain_float(c) && f >= 0 && f < c.nframes;
        let (w, h, d) = (
            g.width.max(0) as usize,
            g.height.max(0) as usize,
            g.depth.max(0) as usize,
        );
        let mut sum = 0.0f64;
        for k in 0..az.n {
            let mut w2 = 0.0f64;
            for j in 0..ay.n {
                let mut wv = 0.0f64;
                if fast {
                    let v = c.voxels.as_deref().expect("voxels");
                    let base = elem(
                        w,
                        h,
                        d,
                        0,
                        ay.idx[j] as usize,
                        az.idx[k] as usize,
                        f as usize,
                    );
                    for i in 0..ax.n {
                        let x = sse::widen(fget(v, base + ax.idx[i] as usize));
                        wv = (o.add)(wv, (o.mul)(x, ax.w[i]));
                    }
                } else {
                    for i in 0..ax.n {
                        let x = self.mri_get_vox_val(c, ax.idx[i], ay.idx[j], az.idx[k], f, 0)?;
                        wv = (o.add)(wv, (o.mul)(sse::widen(x), ax.w[i]));
                    }
                }
                w2 = (o.add)((o.mul)(wv, ay.w[j]), w2);
            }
            sum = (o.add)((o.mul)(w2, az.w[k]), sum);
        }
        Ok(sum)
    }

    /// `MRIsampleBSpline(b, x, y, z, frame, &val)` (§4.1): its value and the sample (none where
    /// FS-964 refuses the degree: `ERROR_BADPARM`). A frame outside the coefficients' or a point
    /// outside the volume by more than half a voxel gives the coefficients' outside value.
    pub fn mri_sample_bspline(
        &mut self,
        b: &BSpline,
        x: f64,
        y: f64,
        z: f64,
        frame: i32,
    ) -> Result<(i32, Option<f64>), Stop> {
        let c = b.coeff.as_ref().expect("coefficients");
        if frame < 0 || frame >= c.nframes || self.mri_index_not_in_volume(c, x, y, z) == 1 {
            return Ok((codes::NONE, Some(c.outside_val)));
        }
        let Some(a) = self.sampler_weights(b, c, [x, y, z], "MRIsampleBSpline")? else {
            return Ok((codes::BAD_PARM, None));
        };
        let v = self.bspline_sum(b, c, &a, frame)?;
        Ok((codes::NONE, Some(v)))
    }

    /// `MRIsampleSeqBSpline(b, x, y, z, valvect, first, last)` (§4.2): `valvect[f]` for `f` from
    /// `first` to `last` (indexed by the frame itself: `valvect` holds at least `last + 1`
    /// floats); the frames are not checked against the coefficients'.
    pub fn mri_sample_seq_bspline(
        &mut self,
        b: &BSpline,
        p: [f64; 3],
        valvect: &mut [f32],
        first: i32,
        last: i32,
    ) -> Result<i32, Stop> {
        let c = b.coeff.as_ref().expect("coefficients");
        if self.mri_index_not_in_volume(c, p[0], p[1], p[2]) == 1 {
            let o = sse::narrow(c.outside_val);
            for f in first..=last {
                valvect[f as usize] = o;
            }
            return Ok(codes::NONE);
        }
        let Some(a) = self.sampler_weights(b, c, p, "MRIsampleSeqBSpline")? else {
            return Ok(codes::BAD_PARM);
        };
        for f in first..=last {
            let v = self.bspline_sum(b, c, &a, f)?;
            valvect[f as usize] = sse::narrow(v);
        }
        Ok(codes::NONE)
    }
}
