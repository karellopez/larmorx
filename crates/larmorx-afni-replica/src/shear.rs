// SPDX-License-Identifier: Apache-2.0 AND CC-BY-4.0
// Translated to Rust from AFNI 26.3.05 src/thd_shear3d.c (norm_3shear, permute_3shear,
// permute_dmat33, permute_dfvec3, shear_xzyx, shear_arb, shear_best, rot_to_shear,
// rot_to_shear_matvec, DMAT_svd, DMAT_svdrot_newer) and the MCW_3shear struct of
// src/thd_shear3d.h. Major portions Copyright (C) 1994-2000 Medical College of Wisconsin (AFNI,
// Robert W. Cox et al.), released by AFNI 26.3.05 under the Creative Commons Attribution 4.0
// International licence (CC BY 4.0).
// CC BY 4.0: https://creativecommons.org/licenses/by/4.0/ (the material is provided as is, without
// warranties of any kind); source: https://github.com/afni/afni, tag AFNI_26.3.05.
// AFNI asks that work based on it cite its authors (LICENSE.txt).
// Rust translation Copyright 2026 Karel Lopez Vilaret, licensed under the Apache License 2.0;
// modified from the original: translated to Rust, with the changes PROVENANCE.md lists.
//! The four-shear decomposition of a rotation plus a shift (`MCW_3shear`), as AFNI computes it
//! to rotate a volume: a rotation matrix `Q` (with `det Q = 1`) and shift `d`, in voxel units,
//! factored as `Q = Sx2 · Sz · Sy · Sx1`, each `S` a one-dimensional shear (with the shifts on
//! the last three), for the six orders of the axes, keeping the one with the smallest largest
//! shear factor ([`shear_best`]); a preliminary flip of two axes when the trace of `Q` is below 1.
//!
//! The decomposition is the closed form Maple wrote for AFNI (`shear_xzyx`), translated
//! operation by operation (all in double; the cube root as `pow(t, 0.333333333333333)`, glibc's
//! with `reproduce`, [`Libc`]).

use crate::eispack::{SvdError, svd_double3};
use crate::vecmat::{self, DMat33, DVec3, Libc};

/// `MCW_3shear`: four shears, shear `i` along axis `ax[i]` (0 x, 1 y, 2 z) with scale factors
/// `scl[i][j]` along direction `j` and shift `sft[i]`, after flipping axes `flip0` and `flip1`
/// when `flip0 >= 0`. `ax[0] < 0` marks an invalid set (`ISVALID_3SHEAR`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shear {
    pub ax: [i32; 4],
    pub flip0: i32,
    pub flip1: i32,
    pub scl: [[f64; 3]; 4],
    pub sft: [f64; 4],
}

impl Shear {
    /// `ZZME(shr)` then `INVALIDATE_3SHEAR(shr)`.
    fn invalid() -> Shear {
        Shear {
            ax: [-1, 0, 0, 0],
            flip0: 0,
            flip1: 0,
            scl: [[0.0; 3]; 4],
            sft: [0.0; 4],
        }
    }

    /// `ISVALID_3SHEAR`.
    pub fn is_valid(&self) -> bool {
        self.ax[0] >= 0
    }
}

/// `BIG_NORM`.
const BIG_NORM: f64 = 1.0e38;

/// `norm_3shear`: the largest off-axis scale factor of the first three shears.
pub fn norm_3shear(sh: &Shear) -> f64 {
    if !sh.is_valid() {
        return BIG_NORM;
    }
    let mut top = 0.0f64;
    for ii in 0..3 {
        let jj = sh.ax[ii] as usize;
        let val = sh.scl[ii][(jj + 1) % 3].abs();
        if val > top {
            top = val;
        }
        let val = sh.scl[ii][(jj + 2) % 3].abs();
        if val > top {
            top = val;
        }
    }
    top
}

/// `permute_3shear`.
fn permute_3shear(shin: &Shear, ox: [usize; 3]) -> Shear {
    if !shin.is_valid() {
        return Shear::invalid();
    }
    let mut out = Shear {
        ax: [0; 4],
        flip0: 0,
        flip1: 0,
        scl: [[0.0; 3]; 4],
        sft: [0.0; 4],
    };
    for ii in 0..4 {
        let ain = shin.ax[ii] as usize;
        out.ax[ii] = ox[ain] as i32;
        out.scl[ii][ox[0]] = shin.scl[ii][0];
        out.scl[ii][ox[1]] = shin.scl[ii][1];
        out.scl[ii][ox[2]] = shin.scl[ii][2];
        out.sft[ii] = shin.sft[ii];
    }
    out.flip0 = shin.flip0;
    out.flip1 = shin.flip1;
    out
}

/// `permute_dmat33`: `m[i][j] = q[pi[i]][pi[j]]`.
fn permute_dmat33(q: &DMat33, ox: [usize; 3]) -> DMat33 {
    let mut m = [[0.0; 3]; 3];
    for ii in 0..3 {
        for jj in 0..3 {
            m[ii][jj] = q[ox[ii]][ox[jj]];
        }
    }
    m
}

/// `permute_dfvec3`.
fn permute_dfvec3(q: &DVec3, ox: [usize; 3]) -> DVec3 {
    [q[ox[0]], q[ox[1]], q[ox[2]]]
}

/// `shear_xzyx`: `Q = Sx[bx2,cx2,f] Sz[az,bz,f] Sy[ay,cy,f] Sx[bx1,cx1,1]` with the shifts, by
/// Maple's closed form; invalid where a division would be by zero.
#[allow(clippy::many_single_char_names, clippy::too_many_lines)]
pub fn shear_xzyx(q: &DMat33, xyzdel: &DVec3, libc: Libc) -> Shear {
    let mut shr = Shear::invalid();
    let [[q11, q12, q13], [q21, q22, q23], [q31, q32, q33]] = *q;
    let (xdel, ydel, zdel) = (xyzdel[0], xyzdel[1], xyzdel[2]);

    let ay = q21;
    let dy = ydel;
    let t1 = q21 * q12;
    let t3 = q13 * q22;
    let t4 = t3 * q31;
    let t5 = q21 * q13;
    let t6 = t5 * q32;
    let t7 = q23 * q11;
    let t8 = t7 * q32;
    let t9 = q12 * q23;
    let t10 = t9 * q31;
    let t11 = q22 * q11;
    let t12 = t11 * q33;
    let t13 = t1 * q33 + t4 - t6 + t8 - t10 - t12;
    let t15 = q32 * q32;
    let t16 = t15 * q32;
    let t17 = q21 * q21;
    let t18 = t17 * q21;
    let t19 = t16 * t18;
    let t20 = q22 * q22;
    let t22 = q31 * q31;
    let t23 = t22 * q32;
    let t24 = q21 * t20 * t23;
    let t25 = t20 * q22;
    let t26 = t22 * q31;
    let t27 = t25 * t26;
    let t28 = t15 * t17;
    let t29 = q22 * q31;
    let t30 = t28 * t29;
    let t32 = t13 * t13;

    let mut t34 = (-t19 - 3.0 * t24 + t27 + 3.0 * t30) * t32;
    if t34 > 0.0 {
        t34 = libc.pow(t34, 0.333333333333333);
    } else if t34 < 0.0 {
        t34 = -libc.pow(-t34, 0.333333333333333);
    } else {
        t34 = 0.0;
    }

    if t13 == 0.0 {
        return shr;
    }

    let t35 = 1.0 / t13 * t34;
    let t36 = t35 + q31;
    let t37 = t36 * q21;
    let t38 = q12 * q33;
    let t44 = t36 * q23;
    let t45 = q11 * q32;
    let t47 = t36 * q12;
    let t50 = t36 * q22;
    let t51 = q11 * q33;
    let t53 = q32 * t17;
    let t54 = t53 * q12;
    let t55 = q32 * q21;
    let t57 = q32 * q31;
    let t61 = q32 * q23 * q11 * q31;
    let t62 = q31 * q21;
    let t64 = q22 * q12;
    let t66 = t22 * q23;
    let t67 = t66 * q12;
    let t68 = t22 * q13;
    let t69 = t68 * q22;
    let t70 = t29 * t51;
    let t73 = -t37 * t38 - t36 * q13 * t29 + t37 * q13 * q32 - t44 * t45
        + t47 * q23 * q31
        + t50 * t51
        + t54
        - t55 * t11
        - t57 * t5
        + t61
        + t62 * t38
        - t62 * t64
        - t67
        + t69
        - t70
        + q31 * t20 * q11;
    let t75 = t20 * t22;

    let mut t77 = t28 - 2.0 * t29 * t55 + t75;
    if t77 == 0.0 {
        return shr;
    }
    t77 = 1.0 / t77;

    let cx2 = t73 * t77;
    let t78 = t44 * q31;
    let t79 = t62 * q22;
    let t80 = t62 * q33;
    let t81 = t37 * q33;
    let t84 = t34 * t34;

    if t84 == 0.0 {
        return shr;
    }
    let t85 = 1.0 / t84;

    let cy = (-t78 + t79 - t80 + t81 - t53 + t66) * t32 * t85;
    let t86 = q21 * t22;
    let t87 = t64 * t36;
    let t89 = t17 * q12;
    let t90 = t89 * t36;
    let t92 = t51 * t22;
    let t94 = t68 * q32;
    let t96 = t36 * t36;
    let t102 = t51 * q31;
    let t107 = t11 * t36;
    let t109 = t38 * t22;
    let t113 =
        t86 * t87 - t57 * t90 + 2.0 * t50 * t92 + 2.0 * t37 * t94 + t96 * t22 * t3 + t96 * q31 * t8
            - 3.0 * t24
            - t96 * q22 * t102
            + 3.0 * t30
            + t27
            - 2.0 * t36 * t26 * t3
            - t19
            - t62 * q32 * t107
            - 2.0 * t37 * t109
            - t26 * q21 * t64;
    let t118 = q32 * q22;
    let t119 = t118 * q11;
    let t121 = q11 * t36;
    let t123 = t26 * q13;
    let t125 = t26 * q23;
    let t127 = q33 * t26;
    let t129 = t96 * q12;
    let t131 = t96 * q21;
    let t132 = t38 * q31;
    let t134 = t22 * t22;
    let t141 = q31 * q13 * q32;
    let t145 = -q11 * t15 * t17 * q31 + t23 * t89 + t86 * t119 + t121 * t28 - t123 * t55
        + t125 * t45
        + t1 * t127
        - t129 * t66
        + t131 * t132
        + t134 * q13 * q22
        - t9 * t134
        - 2.0 * t36 * t22 * t8
        - t131 * t141
        - t11 * t127
        + 2.0 * t47 * t125;

    if t34 == 0.0 {
        return shr;
    }
    let t148 = 1.0 / t34;

    if q21 == 0.0 {
        return shr;
    }
    let t150 = 1.0 / q21;

    let t151 = t148 * t77 * t150;
    let bx2 = (t113 + t145) * t13 * t151;
    let az = -t35;
    let f = (-t29 + t55) * t13 * t148;
    let t157 = ydel * q12;
    let t160 = zdel * t17;
    let t163 = ydel * t22;
    let t164 = t163 * q21;
    let t167 = ydel * t26;
    let t185 = xdel * q21;
    let t190 = ydel * q11;
    let t193 =
        -ydel * q22 * t51 * t26 - t157 * q23 * t134 - t160 * t129 * q33 + t164 * t119 + t163 * t54
            - t167 * q21 * q22 * q12
            + ydel * t134 * t3
            + t157 * q21 * q33 * t26
            - t167 * t6
            - 3.0 * ydel * q21 * t75 * q32
            + t167 * t8
            + 3.0 * ydel * t15 * t17 * q22 * q31
            + t185 * t20 * t26
            - ydel * t16 * t18
            - t190 * t28 * q31;
    let t194 = zdel * q21;
    let t195 = t125 * q12;
    let t203 = xdel * t18;
    let t206 = xdel * t17;
    let t207 = q22 * t22;
    let t210 = t160 * q32;
    let t220 = zdel * t18;
    let t221 = q32 * q12;
    let t224 = t123 * q22;
    let t230 = t194 * t96;
    let t233 = t194 * t195 + t194 * t22 * t12 + t160 * t94 - t194 * q32 * t7 * t22
        + t203 * q31 * t15
        - 2.0 * t206 * t207 * q32
        - t210 * t107
        - t194 * t75 * q11
        + t194 * q31 * t20 * q11 * t36
        + t210 * t11 * q31
        - t220 * t221 * q31
        - t194 * t224
        + t160 * t207 * q12
        + ydel * t25 * t26
        - t230 * t8
        - t160 * t109;
    let t238 = t194 * t36;
    let t240 = ydel * t96;
    let t241 = t240 * q21;
    let t252 = ydel * t36;
    let t264 = -t203 * t36 * t15 + t230 * t12 + 2.0 * t238 * t61 - t241 * t141 - t240 * q22 * t102
        + t220 * t221 * t36
        - t160 * q31 * t87
        + 2.0 * t206 * t36 * t29 * q32
        - 2.0 * t252 * t22 * t8
        + t164 * t87
        + t190 * t36 * t17 * t15
        - t240 * t67
        - 2.0 * t252 * t224
        + 2.0 * t252 * q22 * t92
        + t241 * t132;
    let t267 = t160 * t36;
    let t269 = ydel * q31;
    let t275 = t252 * q21;
    let t292 = -2.0 * t238 * t67 + t240 * t69 + 2.0 * t267 * t132
        - t269 * q32 * t90
        - t185 * t36 * t20 * t22
        + 2.0 * t275 * t94
        + t230 * t10
        + 2.0 * t238 * t69
        + 2.0 * t252 * t195
        - t230 * t4
        - 2.0 * t275 * t109
        - 2.0 * t267 * t141
        - 2.0 * t238 * t70
        + t240 * q31 * t8
        - t269 * q21 * t118 * t121
        + t160 * t96 * q13 * q32;
    let dx = -(t193 + t233 + t264 + t292) * t13 * t151;
    let bz = t36 * t150;
    let cx1 = -(t78 + t79 - t80 - t96 * q23 + t81 - t53) * t150 * t32 * t85;
    let dz = (-t252 + t194) * t150;
    let bx1 = -(-t50 + t55) * t150 * t13 * t148;

    shr.ax[3] = 0;
    shr.scl[3] = [f, bx2, cx2];
    shr.sft[3] = dx;
    shr.ax[2] = 2;
    shr.scl[2] = [az, bz, f];
    shr.sft[2] = dz;
    shr.ax[1] = 1;
    shr.scl[1] = [ay, f, cy];
    shr.sft[1] = dy;
    shr.ax[0] = 0;
    shr.scl[0] = [1.0, bx1, cx1];
    shr.sft[0] = 0.0;
    shr.flip0 = -1;
    shr.flip1 = -1;
    shr
}

/// `shear_arb`: the decomposition with the axes in order `ox`.
fn shear_arb(q: &DMat33, xyzdel: &DVec3, ox: [usize; 3], libc: Libc) -> Shear {
    let qq = permute_dmat33(q, ox);
    let xx = permute_dfvec3(xyzdel, ox);
    let sh = shear_xzyx(&qq, &xx, libc);
    if !sh.is_valid() {
        return sh;
    }
    permute_3shear(&sh, ox)
}

/// `shear_best`: the identity (shifts only) when `Q` is the identity within `1e-6`, else the
/// smallest-norm decomposition of the six axis orders (the first of equal norms).
pub fn shear_best(q: &DMat33, xyzdel: &DVec3, libc: Libc) -> Shear {
    let dsum = vecmat::trace(q);
    let esum = q[0][1].abs()
        + q[0][2].abs()
        + q[1][0].abs()
        + q[1][2].abs()
        + q[2][0].abs()
        + q[2][1].abs();
    if dsum >= 2.99999 && esum / dsum < 1.0e-6 {
        let (dx, dy, dz) = (xyzdel[0], xyzdel[1], xyzdel[2]);
        return Shear {
            ax: [0, 1, 2, 0],
            flip0: -1,
            flip1: -1,
            scl: [
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
                [1.0, 0.0, 0.0],
            ],
            sft: [0.0, dy, dz, dx],
        };
    }
    let orders = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    let sh: Vec<Shear> = orders
        .iter()
        .map(|&o| shear_arb(q, xyzdel, o, libc))
        .collect();
    let mut jbest = 0;
    let mut best = BIG_NORM;
    for (ii, s) in sh.iter().enumerate() {
        let val = norm_3shear(s);
        if val < best {
            best = val;
            jbest = ii;
        }
    }
    sh[jbest]
}

/// The preliminary flips of `rot_to_shear` and `rot_to_shear_matvec`: when the trace of `q` is
/// below 1, the two axes other than the one with the largest diagonal entry (the first of
/// equal ones) are flipped if their diagonal entries add to less than -0.02. Returns the
/// flipped matrix and the axes (`-1, -1` for none).
fn flips(q: DMat33) -> (DMat33, i32, i32) {
    if vecmat::trace(&q) >= 1.0 {
        return (q, -1, -1);
    }
    let mut itop = 0;
    let mut top = q[0][0];
    if top < q[1][1] {
        top = q[1][1];
        itop = 1;
    }
    if top < q[2][2] {
        itop = 2;
    }
    let (i1, i2, p) = match itop {
        1 => (0, 2, vecmat::diag(-1.0, 1.0, -1.0)),
        2 => (0, 1, vecmat::diag(-1.0, -1.0, 1.0)),
        _ => (1, 2, vecmat::diag(1.0, -1.0, -1.0)),
    };
    if q[i1][i1] + q[i2][i2] < -0.02 {
        (vecmat::mul(&q, &p), i1 as i32, i2 as i32)
    } else {
        (q, -1, -1)
    }
}

/// `rot_to_shear`'s and `rot_to_shear_matvec`'s common end: the scaling to voxel units, the best
/// decomposition, and once more with a perturbed matrix if none was valid; then the flips.
fn shear_scaled(
    mut q: DMat33,
    mut d: DVec3,
    del: [f64; 3],
    libc: Libc,
    flip0: i32,
    flip1: i32,
) -> Shear {
    let [xdel, ydel, zdel] = del;
    d[0] /= xdel;
    d[1] /= ydel;
    d[2] /= zdel;
    q[0][1] *= ydel / xdel;
    q[0][2] *= zdel / xdel;
    q[1][0] *= xdel / ydel;
    q[1][2] *= zdel / ydel;
    q[2][0] *= xdel / zdel;
    q[2][1] *= ydel / zdel;
    let mut shr = shear_best(&q, &d, libc);
    if !shr.is_valid() {
        // PER0, PER1, PER2: perturb the matrix a little.
        let p = vecmat::rot_to_matrix(0, 1.09e-6, 1, 1.22e-6, 2, 1.37e-6, libc);
        let q2 = vecmat::mul(&q, &p);
        let pt = vecmat::transpose(&p);
        let q2 = vecmat::mul(&pt, &q2);
        shr = shear_best(&q2, &d, libc);
        if !shr.is_valid() {
            return shr;
        }
    }
    shr.flip0 = flip0;
    shr.flip1 = flip1;
    shr
}

/// `DELTA_BEFORE`, `DELTA_FIXED` (`mrilib.h`): how `rot_to_shear` applies its shift. 3dvolreg
/// always passes -1 (`DELTA_AFTER`'s behaviour: the shift after the rotation).
pub const DELTA_AFTER: i32 = 0;
pub const DELTA_BEFORE: i32 = 1;
pub const DELTA_FIXED: i32 = 2;

/// `rot_to_shear(ax1,th1, ax2,th2, ax3,th3, dcode, dx,dy,dz, xdel,ydel,zdel)`.
#[allow(clippy::too_many_arguments)]
pub fn rot_to_shear(
    ax: [i32; 3],
    th: [f64; 3],
    dcode: i32,
    dxyz: DVec3,
    del: [f64; 3],
    libc: Libc,
) -> Shear {
    let q = vecmat::rot_to_matrix(ax[0], th[0], ax[1], th[1], ax[2], th[2], libc);
    let (q, flip0, flip1) = flips(q);
    let mut d = dxyz;
    match dcode {
        DELTA_BEFORE => d = vecmat::matvec(&q, &d),
        DELTA_FIXED => {
            let c = vecmat::matvec(&q, &d);
            d = vecmat::sub(&d, &c);
        }
        _ => {}
    }
    shear_scaled(q, d, del, libc, flip0, flip1)
}

/// `DMAT_svdrot_newer`: `V·Uᵀ` of the SVD `M = U·diag(s)·Vᵀ` (`DMAT_svd`).
pub fn svdrot_newer(m: &DMat33) -> Result<DMat33, SvdError> {
    let mut a = [0.0; 9];
    for jj in 0..3 {
        for ii in 0..3 {
            a[ii + 3 * jj] = m[ii][jj];
        }
    }
    let (_s, u, v) = svd_double3(&a)?;
    let mut um = [[0.0; 3]; 3];
    let mut vm = [[0.0; 3]; 3];
    for jj in 0..3 {
        for ii in 0..3 {
            um[ii][jj] = u[ii + 3 * jj];
            vm[ii][jj] = v[ii + 3 * jj];
        }
    }
    let ut = vecmat::transpose(&um);
    Ok(vecmat::mul(&vm, &ut))
}

/// `rot_to_shear_matvec(rmat, tvec, xdel,ydel,zdel)`: the shears of `x -> rmat·x + tvec`,
/// with `rmat` first made orthogonal (`q = (V·Uᵀ)ᵀ`, `DMAT_svdrot_newer`); the shift always
/// after the rotation.
pub fn rot_to_shear_matvec(
    rmat: &DMat33,
    tvec: &DVec3,
    del: [f64; 3],
    libc: Libc,
) -> Result<Shear, SvdError> {
    let p = svdrot_newer(rmat)?;
    let q = vecmat::transpose(&p);
    let (q, flip0, flip1) = flips(q);
    Ok(shear_scaled(q, *tvec, del, libc, flip0, flip1))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shears applied to a point, in order: each maps its axis's coordinate to
    /// `scl·x + shift` (with the scale along its own axis `f`).
    fn apply(sh: &Shear, p: DVec3) -> DVec3 {
        let mut x = p;
        if sh.flip0 >= 0 {
            x[sh.flip0 as usize] = -x[sh.flip0 as usize];
            x[sh.flip1 as usize] = -x[sh.flip1 as usize];
        }
        for i in 0..4 {
            let a = sh.ax[i] as usize;
            let v = sh.scl[i][0] * x[0] + sh.scl[i][1] * x[1] + sh.scl[i][2] * x[2] + sh.sft[i];
            x[a] = v;
        }
        x
    }

    #[test]
    fn decomposition_reproduces_the_map() {
        let libc = Libc::default();
        for th in [
            [0.05, -0.02, 0.03],
            [0.3, 0.2, -0.4],
            [2.5, 0.1, 0.2],
            [1e-6, 1.1e-6, 0.9e-6],
        ] {
            let sh = rot_to_shear([2, 0, 1], th, -1, [1.0, -2.0, 0.5], [1.0, 1.0, 1.0], libc);
            assert!(sh.is_valid(), "{th:?}");
            let q = vecmat::rot_to_matrix(2, th[0], 0, th[1], 1, th[2], libc);
            for p in [
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
                [3.0, -2.0, 5.0],
            ] {
                let want = vecmat::add(&vecmat::matvec(&q, &p), &[1.0, -2.0, 0.5]);
                let got = apply(&sh, p);
                for k in 0..3 {
                    assert!(
                        (got[k] - want[k]).abs() < 1e-6,
                        "{th:?} {p:?}: {got:?} {want:?}"
                    );
                }
            }
        }
    }
}
