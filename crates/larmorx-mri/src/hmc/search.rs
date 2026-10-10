// SPDX-License-Identifier: Apache-2.0
//! The optimiser (`specs/mcflirt.md` §7): one sweep of line searches, one parameter at a
//! time, each a bracketing phase followed by parabolic/golden refinement, with positions and
//! cost values in float32.

use super::rigid::Params;

/// The parabola through `(a1, y1)`, `(am, ym)`, `(a2, y2)` (§7.3): its vertex, or `None` if it
/// opens downward or is degenerate. Float32 throughout.
fn vertex(a1: f32, y1: f32, am: f32, ym: f32, a2: f32, y2: f32) -> Option<f32> {
    if super::cost::variant() & (1 << 62) != 0 {
        let (a1, y1, am, ym, a2, y2) = (f64::from(a1), f64::from(y1), f64::from(am), f64::from(ym), f64::from(a2), f64::from(y2));
        let p = (am - a2) * (ym - y1) - (am - a1) * (ym - y2);
        let q = -(am * am - a2 * a2) * (ym - y1) + (am * am - a1 * a1) * (ym - y2);
        let h = (am - a2) * (a2 - a1) * (a1 - am);
        if h.abs() > 1e-15 && p / h < 0.0 {
            return None;
        }
        return if p.abs() > 1e-15 { Some((-q / (2.0 * p)) as f32) } else { None };
    }
    let p = (am - a2) * (ym - y1) - (am - a1) * (ym - y2);
    let q = -(am * am - a2 * a2) * (ym - y1) + (am * am - a1 * a1) * (ym - y2);
    let h = (am - a2) * (a2 - a1) * (a1 - am);
    if h.abs() > 1e-15 && p / h < 0.0 {
        return None;
    }
    if p.abs() > 1e-15 {
        Some(-q / (2.0 * p))
    } else {
        None
    }
}

/// The golden-section point on the longer side of the bracket (ties go to `a1`).
fn golden(a1: f32, am: f32, a2: f32) -> f32 {
    let far = if (a1 - am).abs() >= (a2 - am).abs() {
        a1
    } else {
        a2
    };
    0.381_966_0 * far + 0.618_034_0 * am
}

/// A line search from offset 0 with cost `y0` there, along a parameter of tolerance `u`
/// (§7.2). `phi(a)` is the cost at offset `a`. Returns the best offset and its cost.
pub fn line_search(phi: &mut dyn FnMut(f32) -> f32, y0: f32, u: f32) -> (f32, f32) {
    // Bracketing.
    let (mut am, mut ym) = (0.0f32, y0);
    let mut a1 = { let v2 = super::cost::variant2(); if v2 & 64 != 0 { f32::from_bits((10.0 * u).to_bits() - 1) } else if v2 & 128 != 0 { f32::from_bits((10.0 * u).to_bits() + 1) } else { 10.0 * u } };
    let mut y1 = phi(a1);
    if y1 < ym {
        std::mem::swap(&mut am, &mut a1);
        std::mem::swap(&mut ym, &mut y1);
    }
    let mut a2 = am + 1.6 * (am - a1);
    let mut y2 = phi(a2);
    let dir = if am >= a1 { 1.0f32 } else { -1.0 };
    while ym > y2 {
        let limit = am + 3.2 * (a2 - am);
        let a = match vertex(a1, y1, am, ym, a2, y2) {
            Some(a) if (a - a1) * dir >= 0.0 && (a - limit) * dir <= 0.0 => a,
            _ => am + 1.6 * (a2 - a1),
        };
        let y = phi(a);
        if (a - a1) * dir > 0.0 && (am - a) * dir > 0.0 {
            // Between a1 and am.
            if y < ym {
                (a2, y2) = (am, ym);
                (am, ym) = (a, y);
                break;
            }
            (a1, y1) = (a, y);
        } else if y > ym {
            (a2, y2) = (a, y);
            break;
        } else if (a - a2) * dir < 0.0 {
            (a1, y1) = (am, ym);
            (am, ym) = (a, y);
        } else {
            (a1, y1) = (am, ym);
            (am, ym) = (a2, y2);
            (a2, y2) = (a, y);
        }
    }
    // Refinement.
    let dmin = 0.1 * u;
    for _ in 0..100 {
        if !((a2 - a1).abs() / u > 1.0) {
            break;
        }
        let mut a = match vertex(a1, y1, am, ym, a2, y2) {
            Some(a) if a >= a1.min(a2) && a <= a1.max(a2) => a,
            _ => golden(a1, am, a2),
        };
        let eps = if a2 >= a1 { 1.0f32 } else { -1.0 };
        if (a - a1).abs() < dmin {
            a = a1 + eps * dmin;
        }
        if (a - a2).abs() < dmin {
            a = a2 - eps * dmin;
        }
        if (a - am).abs() < dmin {
            a = golden(a1, am, a2);
        }
        if (am - a1).abs() < 0.4 * u {
            a = am + eps * 0.5 * u;
        }
        if (am - a2).abs() < 0.4 * u {
            a = am - eps * 0.5 * u;
        }
        let y = phi(a);
        if (a - am) * (a2 - am) > 0.0 {
            std::mem::swap(&mut a1, &mut a2);
            std::mem::swap(&mut y1, &mut y2);
        }
        if y < ym {
            (a2, y2) = (am, ym);
            (am, ym) = (a, y);
        } else {
            (a1, y1) = (a, y);
        }
    }
    (am, ym)
}

/// One sweep of line searches over the parameters `order` (§7.1) from `start`, each with its
/// tolerance; `cost` evaluates a parameter vector. Returns the final vector and its cost.
pub fn sweep(
    cost: &mut dyn FnMut(&Params) -> f32,
    start: &Params,
    order: &[usize],
    tolerances: &[f32; 12],
) -> (Params, f32) {
    let mut p = *start;
    let mut best = cost(&p);
    for &k in order {
        if best == 0.0 {
            best = cost(&p);
        }
        let base = p;
        let mut phi = |a: f32| {
            let mut q = base;
            q[k] += f64::from(a);
            cost(&q)
        };
        let (a, y) = line_search(&mut phi, best, tolerances[k]);
        p[k] += f64::from(a);
        best = y;
    }
    (p, best)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parabola_vertex() {
        assert_eq!(vertex(-1.0, 1.0, 0.0, 0.0, 1.0, 1.0), Some(0.0));
        let v = vertex(0.0, 4.0, 1.0, 1.0, 3.0, 1.0).unwrap();
        assert!((v - 2.0).abs() < 1e-6, "{v}");
        // Opening downward.
        assert_eq!(vertex(-1.0, -1.0, 0.0, 0.0, 1.0, -1.0), None);
    }

    #[test]
    fn finds_a_quadratic_minimum_to_the_tolerance() {
        for target in [0.0137f32, -0.3, 2.0, 0.0] {
            let mut evals = Vec::new();
            let mut phi = |a: f32| {
                evals.push(a);
                (a - target) * (a - target) + 1.0
            };
            let y0 = phi(0.0);
            let (a, y) = line_search(&mut phi, y0, 0.01);
            assert!((a - target).abs() <= 0.01, "{target}: {a}");
            assert!(y <= y0);
        }
    }

    #[test]
    fn the_first_trial_is_ten_tolerances_away() {
        let mut first = None;
        let mut phi = |a: f32| {
            first.get_or_insert(a);
            a * a
        };
        line_search(&mut phi, 0.0, 0.004);
        assert_eq!(first, Some(10.0 * 0.004f32));
    }
}
