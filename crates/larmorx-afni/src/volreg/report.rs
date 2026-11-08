// SPDX-License-Identifier: Apache-2.0
//! What 3dvolreg reports about each volume (`specs/3dvolreg.md` §4.4, §4.5, §8.8, §10.2,
//! §10.3): the reported parameters, the matrices of `-1Dmatrix_save`, the maximum
//! displacements of the automask's edge voxels and the per-voxel displacements, and the text
//! files' formats.

use crate::cfmt;

use super::grid::{Grid, M3};
use super::rotate::{Trig, mul, mul_vec, rot3, transpose};

/// The reported parameters `roll, pitch, yaw, dS, dL, dP` of a volume registered with angles
/// `radians` (float32) and shifts `shift` (mm along the dataset axes).
pub fn reported(grid: &Grid, radians: [f32; 3], shift: [f32; 3]) -> [f32; 6] {
    let (_, signs) = grid.angle_axes();
    let deg = radians.map(super::fit::to_degrees);
    let [ds, dl, dp] = grid.reported_shifts(shift);
    [
        deg[0] * signs[0],
        deg[1] * signs[1],
        deg[2] * signs[2],
        ds,
        dl,
        dp,
    ]
}

/// The rotation `R` of reported parameters in DICOM order (§4.5), double.
pub fn dicom_rotation(p: &[f32; 6], trig: Trig) -> M3 {
    let k = std::f64::consts::PI / 180.0;
    let a = |v: f32| -k * f64::from(v);
    rot3([2, 0, 1], [a(p[0]), a(p[1]), a(p[2])], trig)
}

/// The inverse of `A` by cofactors (§4.5).
fn inverse(a: &M3) -> M3 {
    let det =
        a[0][0] * a[1][1] * a[2][2] - a[0][0] * a[1][2] * a[2][1] - a[1][0] * a[0][1] * a[2][2]
            + a[1][0] * a[0][2] * a[2][1]
            + a[2][0] * a[0][1] * a[1][2]
            - a[2][0] * a[0][2] * a[1][1];
    let r = 1.0 / det;
    [
        [
            (a[1][1] * a[2][2] - a[1][2] * a[2][1]) * r,
            (-a[0][1] * a[2][2] + a[0][2] * a[2][1]) * r,
            (a[0][1] * a[1][2] - a[0][2] * a[1][1]) * r,
        ],
        [
            (-a[1][0] * a[2][2] + a[1][2] * a[2][0]) * r,
            (a[0][0] * a[2][2] - a[0][2] * a[2][0]) * r,
            (-a[0][0] * a[1][2] + a[0][2] * a[1][0]) * r,
        ],
        [
            (a[1][0] * a[2][1] - a[1][1] * a[2][0]) * r,
            (-a[0][0] * a[2][1] + a[0][1] * a[2][0]) * r,
            (a[0][0] * a[1][1] - a[0][1] * a[1][0]) * r,
        ],
    ]
}

/// The 12 numbers of `-1Dmatrix_save`'s line for reported parameters `p` (§4.5): base to
/// input, DICOM, with `cb` and `ci` the centres of the base and the input.
pub fn matrix_line(p: &[f32; 6], cb: [f32; 3], ci: [f32; 3], trig: Trig) -> [f64; 12] {
    let r = dicom_rotation(p, trig);
    let t = [p[4], p[5], p[3]];
    let w = mul_vec(&r, ci.map(f64::from)).map(|x| x as f32);
    let v: [f32; 3] = std::array::from_fn(|i| t[i] + (cb[i] - w[i]));
    let ri = inverse(&r);
    let u = mul_vec(&ri, v.map(f64::from)).map(|x| -x);
    [
        ri[0][0], ri[0][1], ri[0][2], u[0], ri[1][0], ri[1][1], ri[1][2], u[1], ri[2][0], ri[2][1],
        ri[2][2], u[2],
    ]
}

/// `-1Dmatrix_save`'s text line.
pub fn matrix_text(m: &[f64; 12]) -> String {
    let parts: Vec<String> = m.iter().map(|&x| cfmt::g_width(x, 13, 6)).collect();
    parts.join(" ")
}

/// The rotation and shift of a volume in dataset order for the displacements (§8.8): `P` with
/// the axes' signs (`reproduce`, AFNI-99) or without them (the default).
pub fn dataset_transform(grid: &Grid, p: &[f32; 6], signed: bool, trig: Trig) -> (M3, [f64; 3]) {
    let r = dicom_rotation(p, trig);
    let pm = grid.p(signed);
    let pt = transpose(&pm);
    let rd = mul(&pt, &mul(&r, &pm));
    let t = [p[4], p[5], p[3]].map(f64::from);
    (rd, mul_vec(&pt, t))
}

/// The displacement `u = x − (R·(x − c) + t + c)` of a position `x` (§8.8).
#[inline]
pub fn displacement(rd: &M3, td: &[f64; 3], c: &[f64; 3], x: [f64; 3]) -> [f64; 3] {
    let d = [x[0] - c[0], x[1] - c[1], x[2] - c[2]];
    let q = mul_vec(rd, d);
    let q = [
        q[0] + td[0] + c[0],
        q[1] + td[1] + c[1],
        q[2] + td[2] + c[2],
    ];
    [x[0] - q[0], x[1] - q[1], x[2] - q[2]]
}

#[inline]
pub fn length(u: [f64; 3]) -> f32 {
    (u[0] * u[0] + u[1] * u[1] + u[2] * u[2]).sqrt() as f32
}

/// The maximum displacements of a run (§8.8).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MaxDisp {
    /// Per volume: the largest displacement and the largest change from the previous volume.
    pub per_volume: Vec<(f32, f32)>,
    /// The largest of each and the first volume reaching it.
    pub max: (f32, usize),
    pub max_delta: (f32, usize),
}

/// The maximum displacements of the edge voxels at `positions` (dataset coordinates) for the
/// volumes' reported parameters `params`.
pub fn max_displacements(
    grid: &Grid,
    positions: &[[f32; 3]],
    params: &[[f32; 6]],
    signed: bool,
    trig: Trig,
) -> MaxDisp {
    use rayon::prelude::*;
    let c = grid.axis_centre().map(f64::from);
    let vectors = |p: &[f32; 6]| -> Vec<[f64; 3]> {
        let (rd, td) = dataset_transform(grid, p, signed, trig);
        positions
            .par_iter()
            .map(|x| displacement(&rd, &td, &c, x.map(f64::from)))
            .collect()
    };
    let mut out = MaxDisp::default();
    let mut prev: Option<Vec<[f64; 3]>> = None;
    for (k, p) in params.iter().enumerate() {
        let u = vectors(p);
        let dmax = u.iter().fold(0f32, |m, &v| m.max(length(v)));
        let emax = match &prev {
            None => 0.0,
            Some(pv) => pv.iter().zip(&u).fold(0f32, |m, (a, b)| {
                m.max(length([a[0] - b[0], a[1] - b[1], a[2] - b[2]]))
            }),
        };
        if dmax > out.max.0 {
            out.max = (dmax, k);
        }
        if emax > out.max_delta.0 {
            out.max_delta = (emax, k);
        }
        out.per_volume.push((dmax, emax));
        prev = Some(u);
    }
    out
}

/// `-1Dfile`'s line.
pub fn oned_line(p: &[f32; 6]) -> String {
    p.iter()
        .map(|&v| cfmt::fixed_width(f64::from(v), 8, 4))
        .collect::<Vec<_>>()
        .join(" ")
}

/// `-dfile`'s line.
pub fn dfile_line(k: usize, p: &[f32; 6], rms: [f32; 2]) -> String {
    format!(
        "{k:>4} {}  {} {}",
        oned_line(p),
        cfmt::g_width(f64::from(rms[0]), 11, 4),
        cfmt::g_width(f64::from(rms[1]), 11, 4)
    )
}

/// `-rotcom`'s line of a volume.
pub fn rotcom_line(method: &str, clip: bool, zpad: usize, p: &[f32; 6]) -> String {
    let f = |v: f32| cfmt::fixed(f64::from(v), 4);
    let pad = if zpad != 0 {
        format!(" -zpad {zpad}")
    } else {
        String::new()
    };
    format!(
        "3drotate {method} {}{pad} -rotate {}I {}R {}A -ashift {}S {}L {}P",
        if clip { "-clipit" } else { "-noclip" },
        f(p[0]),
        f(p[1]),
        f(p[2]),
        f(p[3]),
        f(p[4]),
        f(p[5])
    )
}

/// `-SAVEDISP`'s sixteen products (§10.3): each dataset's suffix and the counts of X, Y and Z.
pub const SAVEDISP_PRODUCTS: [(&str, [usize; 3]); 16] = [
    ("DXX", [2, 0, 0]),
    ("DYY", [0, 2, 0]),
    ("DZZ", [0, 0, 2]),
    ("DXY", [1, 1, 0]),
    ("DXZ", [1, 0, 1]),
    ("DYZ", [0, 1, 1]),
    ("DXXX", [3, 0, 0]),
    ("DYYY", [0, 3, 0]),
    ("DZZZ", [0, 0, 3]),
    ("DXXY", [2, 1, 0]),
    ("DXXZ", [2, 0, 1]),
    ("DXYY", [1, 2, 0]),
    ("DXZZ", [1, 0, 2]),
    ("DYYZ", [0, 2, 1]),
    ("DYZZ", [0, 1, 2]),
    ("DXYZ", [1, 1, 1]),
];

/// The Legendre polynomial `P_a(x)` of §10.3 in float32, left to right.
fn legendre(x: f32, a: usize) -> f32 {
    match a {
        0 => 1.0,
        1 => x,
        2 => ((1.5 * x) * x) - 0.5,
        _ => (((2.5 * x) * x) - 1.5) * x,
    }
}

/// The line of `-SAVEDISP`'s detrending (§10.3 step 1) of one voxel's series `v` (at least 3
/// values): the double sums `S0` (from `v_0`) and `S1` (from 0, float32 products `v_i·i`), then
/// the intercept and slope in the spec's order of operations, each rounded to float32. The
/// order decides the float32 values now and then (the textbook forms change some of them), and
/// the normalisation magnifies such a step (spec §10.3, "The arithmetic, exactly").
fn savedisp_line(v: &[f32]) -> (f32, f32) {
    let mut s0 = f64::from(v[0]);
    for &x in &v[1..] {
        s0 += f64::from(x);
    }
    let mut s1 = 0f64;
    for (i, &x) in v.iter().enumerate().skip(1) {
        s1 += f64::from(x * i as f32);
    }
    let n = v.len();
    let nf = n as f64;
    let q = (n * n) as f64;
    let a = ((2.0 / (nf + 1.0)) * (1.0 / nf)) * (((2.0 * (nf * s0)) - 3.0 * s1) - s0);
    let b = ((-6.0 / (q - 1.0)) * (1.0 / nf)) * (((-s0) - 2.0 * s1) + nf * s0);
    (a as f32, b as f32)
}

/// `-SAVEDISP`'s normalisation (§10.3 steps 1 to 3) of a series of volumes of `total` voxels,
/// in place: voxel by voxel over the volumes, the linear trend removed (skipped with fewer than
/// 3 volumes; `v_i − (a + b·i)` in float32, `b·i` first), then every value multiplied by
/// `1/max |value|` (float32; left alone when the maximum is 0).
pub fn normalise_displacements(series: &mut [f32], total: usize) {
    let nt = series.len().checked_div(total).unwrap_or(0);
    let mut v = vec![0f32; nt];
    for p in 0..total {
        for (t, x) in v.iter_mut().enumerate() {
            *x = series[t * total + p];
        }
        if nt >= 3 {
            let (a, b) = savedisp_line(&v);
            for (i, x) in v.iter_mut().enumerate() {
                *x -= a + b * i as f32;
            }
        }
        let max = v.iter().fold(0f32, |m, &x| m.max(x.abs()));
        if max != 0.0 {
            let r = 1.0f32 / max;
            for x in v.iter_mut() {
                *x *= r;
            }
        }
        for (t, &x) in v.iter().enumerate() {
            series[t * total + p] = x;
        }
    }
}

/// One of `-SAVEDISP`'s products of the normalised displacements `d` (X, Y, Z): each voxel
/// `P_a(X)·P_b(Y)·P_c(Z)`, normalised as the three are.
pub fn savedisp_product(d: &[Vec<f32>; 3], counts: [usize; 3], total: usize) -> Vec<f32> {
    let mut out: Vec<f32> = (0..d[0].len())
        .map(|q| {
            (legendre(d[0][q], counts[0]) * legendre(d[1][q], counts[1]))
                * legendre(d[2][q], counts[2])
        })
        .collect();
    normalise_displacements(&mut out, total);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::volreg::grid::Orient;

    #[test]
    fn the_savedisp_line_of_a_line_is_exact() {
        let v: Vec<f32> = (0..20).map(|i| 3.0 + 0.5 * i as f32).collect();
        assert_eq!(savedisp_line(&v), (3.0, 0.5));
        let mut s = v.clone();
        normalise_displacements(&mut s, 1);
        assert!(s.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn the_base_line_has_negative_zero_translations() {
        let g = Grid {
            n: [40, 44, 24],
            orient: [Orient::R2L, Orient::A2P, Orient::I2S],
            delta: [3.0, 3.0, 3.5],
            origin: [-58.5, -64.5, -40.25],
        };
        let p = reported(&g, [0.0; 3], [0.0; 3]);
        assert_eq!(
            oned_line(&p),
            "  0.0000   0.0000   0.0000   0.0000   0.0000   0.0000"
        );
        let c = g.centre();
        let m = matrix_line(&p, c, c, Trig::Correct);
        let text = matrix_text(&m);
        let words: Vec<&str> = text.split_whitespace().collect();
        assert_eq!(
            words,
            [
                "1", "0", "0", "-0", "0", "1", "0", "-0", "0", "0", "1", "-0"
            ]
        );
    }
}
