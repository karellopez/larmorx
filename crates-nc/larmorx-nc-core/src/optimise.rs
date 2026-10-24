// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.17: fsl-miscmaths 2203.2 (optimise.{h,cc}). Copyright (C)
// 1999-2018 University of Oxford (Mark Jenkinson). Rust translation Copyright 2026 Karel Lopez
// Vilaret; modified from the original. Distributed under the FSL Licence (see LICENSE).
//! `MISCMATHS::optimise`: line searches along the coordinate directions (bracketing, then
//! parabolic steps with golden-section fallbacks), in single precision as FSL computes them.

use crate::arma::Mat;

/// `estquadmin(xnew, x1, xmid, x2, y1, ymid, y2)`: the parabola's minimum through three points.
fn estquadmin(x1: f32, xmid: f32, x2: f32, y1: f32, ymid: f32, y2: f32) -> (bool, f32) {
    let ad = (xmid - x2) * (ymid - y1) - (xmid - x1) * (ymid - y2);
    let bd = -(xmid * xmid - x2 * x2) * (ymid - y1) + (xmid * xmid - x1 * x1) * (ymid - y2);
    let det = (xmid - x2) * (x2 - x1) * (x1 - xmid);
    if f64::from(det.abs()) > 1e-15 && ad / det < 0.0 {
        return (false, 0.0);
    }
    if f64::from(ad.abs()) > 1e-15 {
        (true, -bd / (2.0 * ad))
    } else {
        (false, 0.0)
    }
}

/// `extrapolatept(x1, xmid, x2)`: a golden-ratio step into the larger side.
fn extrapolatept(x1: f32, xmid: f32, x2: f32) -> f32 {
    let extensionratio: f32 = 0.381_966;
    if (x2 - xmid).abs() > (x1 - xmid).abs() {
        extensionratio * x2 + (1.0 - extensionratio) * xmid
    } else {
        extensionratio * x1 + (1.0 - extensionratio) * xmid
    }
}

/// `nextpt(x1, xmid, x2, y1, ymid, y2)`.
fn nextpt(x1: f32, xmid: f32, x2: f32, y1: f32, ymid: f32, y2: f32) -> f32 {
    let (quadok, xnew) = estquadmin(x1, xmid, x2, y1, ymid, y2);
    let lo = if x1 < x2 { x1 } else { x2 };
    let hi = if x1 > x2 { x1 } else { x2 };
    if !quadok || xnew < lo || xnew > hi {
        extrapolatept(x1, xmid, x2)
    } else {
        xnew
    }
}

/// `x * unitdir + pt` (each element `unitdir[i] * (double)x + pt[i]`).
fn along(x: f32, unitdir: &Mat, pt: &Mat) -> Mat {
    unitdir.scale(f64::from(x)).plus(pt)
}

/// The bracket found by `findinitialbound` and the messages it printed.
struct Bracket {
    x1: f32,
    xmid: f32,
    x2: f32,
    y1: f32,
    ymid: f32,
    y2: f32,
}

#[allow(clippy::too_many_arguments)]
fn findinitialbound(
    mut b: Bracket,
    func: &mut dyn FnMut(&Mat) -> f32,
    unitdir: &Mat,
    pt: &Mat,
    messages: &mut Vec<String>,
) -> Bracket {
    let extrapolationfactor: f32 = 1.6;
    let maxextrap: f32 = extrapolationfactor * 2.0;
    if b.y1 == 0.0 {
        b.y1 = func(&along(b.x1, unitdir, pt));
    }
    if b.ymid == 0.0 {
        b.ymid = func(&along(b.xmid, unitdir, pt));
    }
    if b.y1 < b.ymid {
        std::mem::swap(&mut b.x1, &mut b.xmid);
        std::mem::swap(&mut b.y1, &mut b.ymid);
    }
    let mut dir: f32 = 1.0;
    if b.xmid < b.x1 {
        dir = -1.0;
    }
    b.x2 = b.xmid + extrapolationfactor * (b.xmid - b.x1);
    b.y2 = func(&along(b.x2, unitdir, pt));
    while b.ymid > b.y2 {
        let maxx2 = b.xmid + maxextrap * (b.x2 - b.xmid);
        let (quadok, mut newx2) = estquadmin(b.x1, b.xmid, b.x2, b.y1, b.ymid, b.y2);
        if !quadok || (newx2 - b.x1) * dir < 0.0 || (newx2 - maxx2) * dir > 0.0 {
            newx2 = b.xmid + extrapolationfactor * (b.x2 - b.x1);
        }
        let newy2 = func(&along(newx2, unitdir, pt));
        if (newx2 - b.xmid) * (newx2 - b.x1) < 0.0 {
            if newy2 < b.ymid {
                b.x2 = b.xmid;
                b.y2 = b.ymid;
                b.xmid = newx2;
                b.ymid = newy2;
                break;
            } else {
                b.x1 = newx2;
                b.y1 = newy2;
            }
        } else if newy2 > b.ymid {
            b.x2 = newx2;
            b.y2 = newy2;
            break;
        } else if (newx2 - b.x2) * dir < 0.0 {
            b.x1 = b.xmid;
            b.y1 = b.ymid;
            b.xmid = newx2;
            b.ymid = newy2;
        } else {
            b.x1 = b.xmid;
            b.y1 = b.ymid;
            b.xmid = b.x2;
            b.ymid = b.y2;
            b.x2 = newx2;
            b.y2 = newy2;
        }
    }
    if b.y2 < b.ymid || b.y1 < b.ymid {
        messages.push("findinitialbound failed to bracket: current triplet is".to_owned());
    }
    b
}

/// `optimise1d(pt, dir, tol, iterations_done, func, max_iter, init_value, boundguess)`: one line
/// search along `dir`; `pt` moves to the best point, whose cost is returned. `init_value` is the
/// cost at `pt` (0 means unknown: it is evaluated).
#[allow(clippy::too_many_arguments)]
pub fn optimise1d(
    pt: &mut Mat,
    dir: &Mat,
    tol: &Mat,
    func: &mut dyn FnMut(&Mat) -> f32,
    max_iter: i32,
    init_value: &mut f32,
    boundguess: f32,
    messages: &mut Vec<String>,
) -> (f32, i32) {
    let unitdir = dir.div_scalar(dir.sum_square().sqrt());
    let mut dir_tol: f32 = 0.0;
    for n in 0..tol.len() {
        if tol.get(n).abs() > 1e-15 {
            dir_tol = (f64::from(dir_tol) + (unitdir.get(n) / tol.get(n)).abs()) as f32;
        }
    }
    let unittol: f32 = (1.0 / dir_tol).abs();
    let xmid: f32 = 0.0;
    let x1: f32 = boundguess * unittol;
    if *init_value == 0.0 {
        *init_value = func(&along(xmid, &unitdir, pt));
    }
    let ymid = *init_value;
    let y1 = func(&along(x1, &unitdir, pt));
    let b = findinitialbound(
        Bracket {
            x1,
            xmid,
            x2: 0.0,
            y1,
            ymid,
            y2: 0.0,
        },
        func,
        &unitdir,
        pt,
        messages,
    );
    let (mut x1, mut xmid, mut x2, mut y1, mut ymid, mut y2) = (b.x1, b.xmid, b.x2, b.y1, b.ymid, b.y2);
    let min_dist = (0.1 * f64::from(unittol)) as f32;
    let mut it = 0;
    loop {
        it += 1;
        // while ((++it) <= max_iter && fabs((x2-x1)/unittol) > 1.0): NaN stops the loop.
        let span = f64::from(((x2 - x1) / unittol).abs());
        #[allow(clippy::neg_cmp_op_on_partial_ord)]
        if it > max_iter || !(span > 1.0) {
            break;
        }
        let mut xnew = if it > 0 {
            nextpt(x1, xmid, x2, y1, ymid, y2)
        } else {
            extrapolatept(x1, xmid, x2)
        };
        let mut dirn: f32 = 1.0;
        if x2 < x1 {
            dirn = -1.0;
        }
        if (xnew - x1).abs() < min_dist {
            xnew = x1 + dirn * min_dist;
        }
        if (xnew - x2).abs() < min_dist {
            xnew = x2 - dirn * min_dist;
        }
        if (xnew - xmid).abs() < min_dist {
            xnew = extrapolatept(x1, xmid, x2);
        }
        if f64::from((xmid - x1).abs()) < 0.4 * f64::from(unittol) {
            xnew = (f64::from(xmid) + f64::from(dirn) * 0.5 * f64::from(unittol)) as f32;
        }
        if f64::from((xmid - x2).abs()) < 0.4 * f64::from(unittol) {
            xnew = (f64::from(xmid) - f64::from(dirn) * 0.5 * f64::from(unittol)) as f32;
        }
        let ynew = func(&along(xnew, &unitdir, pt));
        if (xnew - xmid) * (x2 - xmid) > 0.0 {
            std::mem::swap(&mut x1, &mut x2);
            std::mem::swap(&mut y1, &mut y2);
        }
        if ynew < ymid {
            x2 = xmid;
            y2 = ymid;
            xmid = xnew;
            ymid = ynew;
        } else {
            x1 = xnew;
            y1 = ynew;
        }
    }
    *pt = along(xmid, &unitdir, pt);
    (ymid, it)
}

/// `optimise(pt, numopt, tol, func, iterations_done, max_iter, boundguess, "brent")`: up to
/// `max_iter` sweeps of line searches along the first `numopt` coordinate directions (mcflirt
/// runs one sweep). Returns the final cost.
pub fn optimise(
    pt: &mut Mat,
    numopt: usize,
    tol: &Mat,
    func: &mut dyn FnMut(&Mat) -> f32,
    max_iter: i32,
    boundguess: &[f64],
    messages: &mut Vec<String>,
) -> f32 {
    let n = pt.len();
    let mut fval: f32 = 0.0;
    let mut it = 0;
    loop {
        it += 1;
        if it > max_iter {
            break;
        }
        let initpt = pt.clone();
        let bi = (it as usize).min(boundguess.len()) - 1;
        let bndguess = boundguess[bi] as f32;
        for d in 0..numopt {
            let mut dir = Mat::zeros(n, 1);
            dir.put(d, 1.0);
            let mut init = fval;
            let (fval2, _) = optimise1d(pt, &dir, tol, func, 100, &mut init, bndguess, messages);
            fval = fval2;
        }
        // The convergence test: no effect with max_iter = 1 except stopping the loop.
        let mut inv_tol = vec![0.0f64; tol.len()];
        for (k, v) in inv_tol.iter_mut().enumerate() {
            if tol.get(k).abs() > 1e-15 {
                *v = (1.0 / tol.get(k)).abs();
            }
            *v /= tol.len() as f32 as f64;
        }
        let mut avtol: f64 = 0.0;
        for k in 0..n {
            avtol += ((initpt.get(k) - pt.get(k)) * inv_tol[k]).abs();
        }
        if (avtol as f32) < 1.0 {
            break;
        }
    }
    fval
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_parabola_minimum() {
        let mut pt = Mat::col(&[0.0, 0.0]);
        let tol = Mat::col(&[0.001, 0.001]);
        let mut f = |p: &Mat| ((p.get(0) - 0.3).powi(2) + (p.get(1) + 0.2).powi(2) + 1.0) as f32;
        let mut msgs = Vec::new();
        let v = optimise(&mut pt, 2, &tol, &mut f, 1, &[10.0, 1.0], &mut msgs);
        assert!((pt.get(0) - 0.3).abs() < 0.01, "{:?}", pt);
        assert!((pt.get(1) + 0.2).abs() < 0.01, "{:?}", pt);
        assert!((v - 1.0).abs() < 1e-3);
    }
}
