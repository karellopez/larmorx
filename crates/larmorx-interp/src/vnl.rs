//! vnl's error function, as ITK's Gaussian interpolators use it.
//!
//! `vnl_erf(x) = sign(x) · P(½, x²)`, with the regularised incomplete gamma function `P`
//! computed by a series or a continued fraction to a relative error of 3e-7, and a Lanczos
//! approximation of log Γ. Ported from VXL's `core/vnl/vnl_gamma.cxx` and `vnl_erf.h` (BSD
//! licence) as bundled with ITK v5.4.5, in the same operation order, so interpolated values
//! match ITK's rather than an exact erf.

// The constants are written exactly as in vnl, so they parse to the same doubles.
#[allow(clippy::excessive_precision)]
fn log_gamma(x: f64) -> f64 {
    let mut zp = 2.50662827563479526904;
    zp += 225.525584619175212544 / x;
    zp -= 268.295973841304927459 / (x + 1.0);
    zp += 80.9030806934622512966 / (x + 2.0);
    zp -= 5.00757863970517583837 / (x + 3.0);
    zp += 0.0114684895434781459556 / (x + 4.0);
    let x1 = x + 4.65;
    libm::log(zp) + (x - 0.5) * libm::log(x1) - x1
}

const MAX_ITS: usize = 100;
const MAX_REL_ERROR: f64 = 3.0e-7;
const VERY_SMALL: f64 = 1.0e-30;

fn gamma_series(a: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    let mut a_i = a;
    let mut term = 1.0 / a;
    let mut sum = term;
    for _ in 1..=MAX_ITS {
        a_i += 1.0;
        term *= x / a_i;
        sum += term;
        if term.abs() < sum.abs() * MAX_REL_ERROR {
            break;
        }
    }
    sum * libm::exp(-x + a * libm::log(x) - log_gamma(a))
}

fn gamma_cont_frac(a: f64, x: f64) -> f64 {
    let mut b_i = x + 1.0 - a;
    let mut c = 1.0 / VERY_SMALL;
    let mut d = 1.0 / b_i;
    let mut cf = d;
    for i in 1..=MAX_ITS {
        let i = i as f64;
        let a_i = i * (a - i);
        b_i += 2.0;
        d = a_i * d + b_i;
        if d.abs() < VERY_SMALL {
            d = VERY_SMALL;
        }
        c = b_i + a_i / c;
        if c.abs() < VERY_SMALL {
            c = VERY_SMALL;
        }
        d = 1.0 / d;
        let delta = d * c;
        cf *= delta;
        if (delta - 1.0).abs() < MAX_REL_ERROR {
            break;
        }
    }
    libm::exp(-x + a * libm::log(x) - log_gamma(a)) * cf
}

fn gamma_p(a: f64, x: f64) -> f64 {
    if x < a + 1.0 {
        gamma_series(a, x)
    } else {
        1.0 - gamma_cont_frac(a, x)
    }
}

/// `vnl_erf`.
pub fn erf(x: f64) -> f64 {
    if x < 0.0 {
        -gamma_p(0.5, x * x)
    } else {
        gamma_p(0.5, x * x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_to_the_exact_erf() {
        for x in [-3.0, -1.2, -0.3, 0.0, 0.1, 0.7, 1.0, 2.5, 6.0] {
            let (ours, exact) = (erf(x), libm::erf(x));
            assert!(
                (ours - exact).abs() <= 1e-6 * exact.abs().max(1e-3),
                "{x}: {ours} vs {exact}"
            );
        }
        assert_eq!(erf(0.0), 0.0);
    }
}
