// SPDX-License-Identifier: Apache-2.0
//! ITK's `TDistribution` and `GaussianDistribution` inverse CDFs (ITK v5.4.5,
//! `Modules/Numerics/Statistics/src/itkTDistribution.cxx`, `itkGaussianDistribution.cxx`), as
//! the GrubbsRosner outlier filter uses them, with the SLATEC routines ITK bundles for the
//! incomplete beta and gamma functions (`Modules/ThirdParty/Netlib/src/netlib/slatec/`:
//! `dbetai`, `dlbeta`, `dgamma`, `dlngam`, `d9lgmc`, `dlnrel`, `dgamlm`, `dcsevl`, `initds`,
//! `d1mach`; the SLATEC library is in the public domain), translated statement by statement
//! from their f2c versions. Logarithms, exponentials and sines are correctly rounded
//! (`larmorx_core::math`); SLATEC's error messages (`xermsg`, printed by ITK on standard error
//! and otherwise ignored) are collected in [`Slatec::messages`].

use larmorx_core::math::{exp, log, sin};

/// The SLATEC messages a computation printed.
#[derive(Clone, Debug, Default)]
pub struct Slatec {
    pub messages: Vec<String>,
}

fn d1mach(i: i32) -> f64 {
    match i {
        1 => f64::MIN_POSITIVE,
        2 => f64::MAX,
        3 => f64::EPSILON / 2.0,
        4 => f64::EPSILON,
        _ => 0.0,
    }
}

/// `d_int`: truncation toward zero.
fn d_int(x: f64) -> f64 {
    if x > 0.0 { x.floor() } else { -(-x).floor() }
}

const GAMCS: [f64; 42] = [
    0.008571195590989331421920062399942,
    0.004415381324841006757191315771652,
    0.05685043681599363378632664588789,
    -0.004219835396418560501012500186624,
    0.001326808181212460220584006796352,
    -1.893024529798880432523947023886e-4,
    3.606925327441245256578082217225e-5,
    -6.056761904460864218485548290365e-6,
    1.055829546302283344731823509093e-6,
    -1.811967365542384048291855891166e-7,
    3.117724964715322277790254593169e-8,
    -5.354219639019687140874081024347e-9,
    9.19327551985958894688778682594e-10,
    -1.577941280288339761767423273953e-10,
    2.707980622934954543266540433089e-11,
    -4.646818653825730144081661058933e-12,
    7.973350192007419656460767175359e-13,
    -1.368078209830916025799499172309e-13,
    2.347319486563800657233471771688e-14,
    -4.027432614949066932766570534699e-15,
    6.910051747372100912138336975257e-16,
    -1.185584500221992907052387126192e-16,
    2.034148542496373955201026051932e-17,
    -3.490054341717405849274012949108e-18,
    5.987993856485305567135051066026e-19,
    -1.027378057872228074490069778431e-19,
    1.762702816060529824942759660748e-20,
    -3.024320653735306260958772112042e-21,
    5.188914660218397839717833550506e-22,
    -8.902770842456576692449251601066e-23,
    1.527474068493342602274596891306e-23,
    -2.620731256187362900257328332799e-24,
    4.496464047830538670331046570666e-25,
    -7.714712731336877911703901525333e-26,
    1.323635453126044036486572714666e-26,
    -2.270999412942928816702313813333e-27,
    3.896418998003991449320816639999e-28,
    -6.685198115125953327792127999999e-29,
    1.146998663140024384347613866666e-29,
    -1.967938586345134677295103999999e-30,
    3.376448816585338090334890666666e-31,
    -5.793070335782135784625493333333e-32,
];

const ALGMCS: [f64; 15] = [
    0.1666389480451863247205729650822,
    -1.384948176067563840732986059135e-5,
    9.810825646924729426157171547487e-9,
    -1.809129475572494194263306266719e-11,
    6.221098041892605227126015543416e-14,
    -3.399615005417721944303330599666e-16,
    2.683181998482698748957538846666e-18,
    -2.868042435334643284144622399999e-20,
    3.962837061046434803679306666666e-22,
    -6.831888753985766870111999999999e-24,
    1.429227355942498147573333333333e-25,
    -3.547598158101070547199999999999e-27,
    1.025680058010470912e-28,
    -3.401102254316748799999999999999e-30,
    1.276642195630062933333333333333e-31,
];

const ALNRCS: [f64; 43] = [
    1.0378693562743769800686267719098,
    -0.13364301504908918098766041553133,
    0.01940824913552056335792619937475,
    -0.0030107551127535777690376537776592,
    4.8694614797154850090456366509137e-4,
    -8.1054881893175356066809943008622e-5,
    1.3778847799559524782938251496059e-5,
    -2.3802210894358970251369992914935e-6,
    4.1640416213865183476391859901989e-7,
    -7.3595828378075994984266837031998e-8,
    1.3117611876241674949152294345011e-8,
    -2.3546709317742425136696092330175e-9,
    4.2522773276034997775638052962567e-10,
    -7.71908941348407968261081074933e-11,
    1.4075746481359069909215356472191e-11,
    -2.5769072058024680627537078627584e-12,
    4.7342406666294421849154395005938e-13,
    -8.7249012674742641745301263292675e-14,
    1.6124614902740551465739833119115e-14,
    -2.9875652015665773006710792416815e-15,
    5.5480701209082887983041321697279e-16,
    -1.0324619158271569595141333961932e-16,
    1.9250239203049851177878503244868e-17,
    -3.5955073465265150011189707844266e-18,
    6.7264542537876857892194574226773e-19,
    -1.2602624168735219252082425637546e-19,
    2.3644884408606210044916158955519e-20,
    -4.4419377050807936898878389179733e-21,
    8.3546594464034259016241293994666e-22,
    -1.5731559416479562574899253521066e-22,
    2.9653128740247422686154369706666e-23,
    -5.5949583481815947292156013226666e-24,
    1.0566354268835681048187284138666e-24,
    -1.9972483680670204548314999466666e-25,
    3.7782977818839361421049855999999e-26,
    -7.1531586889081740345038165333333e-27,
    1.3552488463674213646502024533333e-27,
    -2.5694673048487567430079829333333e-28,
    4.8747756066216949076459519999999e-29,
    -9.2542112530849715321132373333333e-30,
    1.757859784176023923326976e-30,
    -3.3410026677731010351377066666666e-31,
    6.3533936180236187354180266666666e-32,
];

const SQ2PIL: f64 = 0.91893853320467274178032973640562;
const SQPI2L: f64 = 0.225791352644727432363097614947441;
const PI: f64 = 3.1415926535897932384626433832795;

impl Slatec {
    fn xermsg(&mut self, subrou: &str, messg: &str) {
        self.messages.push(format!("SLATEC/{subrou}: {messg}"));
    }

    /// `initds`: the number of terms of a Chebyshev series for accuracy `eta` (in float).
    fn initds(&mut self, os: &[f64], eta: f32) -> usize {
        let nos = os.len();
        let mut err = 0.0f32;
        let mut i = 0usize;
        for ii in 1..=nos {
            i = nos + 1 - ii;
            err += (os[i - 1] as f32).abs();
            if err > eta {
                break;
            }
        }
        if i == nos {
            self.xermsg("INITDS", "Chebyshev series too short for specified accuracy");
        }
        i
    }

    /// `dcsevl`: the first `n` terms of the Chebyshev series `cs` at `x`.
    fn dcsevl(&mut self, x: f64, cs: &[f64], n: usize) -> f64 {
        let onepl = 1.0 + d1mach(4);
        if n < 1 {
            self.xermsg("DCSEVL", "NUMBER OF TERMS .LE. 0");
        }
        if n > 1000 {
            self.xermsg("DCSEVL", "NUMBER OF TERMS .GT. 1000");
        }
        if x.abs() > onepl {
            self.xermsg("DCSEVL", "X OUTSIDE THE INTERVAL (-1,+1)");
        }
        let (mut b0, mut b1, mut b2) = (0.0f64, 0.0f64, 0.0f64);
        let twox = x * 2.0;
        for i in 1..=n {
            b2 = b1;
            b1 = b0;
            let ni = n + 1 - i;
            b0 = twox * b1 - b2 + cs[ni - 1];
        }
        (b0 - b2) * 0.5
    }

    /// `dgamlm`: the range of arguments for which `dgamma` neither underflows nor
    /// overflows.
    fn dgamlm(&mut self) -> (f64, f64) {
        let alnsml = log(d1mach(1));
        let mut xmin = -alnsml;
        let mut found = false;
        for _ in 1..=10 {
            let xold = xmin;
            let xln = log(xmin);
            xmin -= xmin * ((xmin + 0.5) * xln - xmin - 0.2258 + alnsml) / (xmin * xln + 0.5);
            if (xmin - xold).abs() < 0.005 {
                found = true;
                break;
            }
        }
        if !found {
            self.xermsg("DGAMLM", "UNABLE TO FIND XMIN");
        }
        xmin = -xmin + 0.01;
        let alnbig = log(d1mach(2));
        let mut xmax = alnbig;
        let mut found = false;
        for _ in 1..=10 {
            let xold = xmax;
            let xln = log(xmax);
            xmax -= xmax * ((xmax - 0.5) * xln - xmax + 0.9189 - alnbig) / (xmax * xln - 0.5);
            if (xmax - xold).abs() < 0.005 {
                found = true;
                break;
            }
        }
        if !found {
            self.xermsg("DGAMLM", "UNABLE TO FIND XMAX");
        }
        xmax += -0.01;
        let a = xmin;
        let b = -xmax + 1.0;
        xmin = if a >= b { a } else { b };
        (xmin, xmax)
    }

    /// `d9lgmc`: the log-gamma correction for `x >= 10`.
    fn d9lgmc(&mut self, x: f64) -> f64 {
        let nalgm = self.initds(&ALGMCS, d1mach(3) as f32);
        let xbig = 1.0 / d1mach(3).sqrt();
        let d1 = log(d1mach(2) / 12.0);
        let d2 = -log(d1mach(1) * 12.0);
        let xmax = exp(if d1 <= d2 { d1 } else { d2 });
        if x < 10.0 {
            self.xermsg("D9LGMC", "X MUST BE GE 10");
        }
        if x >= xmax {
            self.xermsg("D9LGMC", "X SO BIG D9LGMC UNDERFLOWS");
            return 0.0;
        }
        let mut r = 1.0 / (x * 12.0);
        if x < xbig {
            let d2 = 10.0 / x;
            let d1 = d2 * d2 * 2.0 - 1.0;
            r = self.dcsevl(d1, &ALGMCS, nalgm) / x;
        }
        r
    }

    /// `dgamma`: the gamma function.
    pub fn dgamma(&mut self, x: f64) -> f64 {
        let ngam = self.initds(&GAMCS, (d1mach(3) as f32) * 0.1f32);
        let (xmin, xmax) = self.dgamlm();
        let dxrel = d1mach(4).sqrt();
        let mut y = x.abs();
        if y > 10.0 {
            if x > xmax {
                self.xermsg("DGAMMA", "X SO BIG GAMMA OVERFLOWS");
            }
            let mut r = 0.0;
            if x < xmin {
                self.xermsg("DGAMMA", "X SO SMALL GAMMA UNDERFLOWS");
                return r;
            }
            r = exp((y - 0.5) * log(y) - y + SQ2PIL + self.d9lgmc(y));
            if x > 0.0 {
                return r;
            }
            if ((x - d_int(x - 0.5)) / x).abs() < dxrel {
                self.xermsg(
                    "DGAMMA",
                    "ANSWER LT HALF PRECISION, X TOO NEAR NEGATIVE INTEGER",
                );
            }
            let sinpiy = sin(PI * y);
            if sinpiy == 0.0 {
                self.xermsg("DGAMMA", "X IS A NEGATIVE INTEGER");
            }
            return -PI / (y * sinpiy * r);
        }
        let mut n = x as i64;
        if x < 0.0 {
            n -= 1;
        }
        y = x - n as f64;
        n -= 1;
        let mut r = self.dcsevl(y * 2.0 - 1.0, &GAMCS, ngam) + 0.9375;
        if n == 0 {
            return r;
        }
        if n > 0 {
            for i in 1..=n {
                r *= y + i as f64;
            }
            return r;
        }
        let n = -n;
        if x == 0.0 {
            self.xermsg("DGAMMA", "X IS 0");
        }
        if x < 0.0 && x + n as f64 - 2.0 == 0.0 {
            self.xermsg("DGAMMA", "X IS A NEGATIVE INTEGER");
        }
        if x < -0.5 && ((x - d_int(x - 0.5)) / x).abs() < dxrel {
            self.xermsg(
                "DGAMMA",
                "ANSWER LT HALF PRECISION BECAUSE X TOO NEAR NEGATIVE INTEGER",
            );
        }
        for i in 1..=n {
            r /= x + i as f64 - 1.0;
        }
        r
    }

    /// `dlngam`: the logarithm of the absolute value of the gamma function.
    fn dlngam(&mut self, x: f64) -> f64 {
        let temp = 1.0 / log(d1mach(2));
        let xmax = temp * d1mach(2);
        let dxrel = d1mach(4).sqrt();
        let y = x.abs();
        if y <= 10.0 {
            return log(self.dgamma(x).abs());
        }
        if y > xmax {
            self.xermsg("DLNGAM", "ABS(X) SO BIG DLNGAM OVERFLOWS");
        }
        if x > 0.0 {
            return SQ2PIL + (x - 0.5) * log(x) - x + self.d9lgmc(y);
        }
        let sinpiy = sin(PI * y).abs();
        if sinpiy == 0.0 {
            self.xermsg("DLNGAM", "X IS A NEGATIVE INTEGER");
        }
        if ((x - d_int(x - 0.5)) / x).abs() < dxrel {
            self.xermsg(
                "DLNGAM",
                "ANSWER LT HALF PRECISION BECAUSE X TOO NEAR NEGATIVE INTEGER",
            );
        }
        SQPI2L + (x - 0.5) * log(y) - x - log(sinpiy) - self.d9lgmc(y)
    }

    /// `dlnrel`: `log(1 + x)`.
    fn dlnrel(&mut self, x: f64) -> f64 {
        let nlnrel = self.initds(&ALNRCS, (d1mach(3) as f32) * 0.1f32);
        let xmin = d1mach(4).sqrt() - 1.0;
        if x <= -1.0 {
            self.xermsg("DLNREL", "X IS LE -1");
        }
        if x < xmin {
            self.xermsg("DLNREL", "ANSWER LT HALF PRECISION BECAUSE X TOO NEAR -1");
        }
        let mut r = 0.0;
        if x.abs() <= 0.375 {
            r = x * (1.0 - x * self.dcsevl(x / 0.375, &ALNRCS, nlnrel));
        }
        if x.abs() > 0.375 {
            r = log(x + 1.0);
        }
        r
    }

    /// `dlbeta`: the logarithm of the complete beta function.
    fn dlbeta(&mut self, a: f64, b: f64) -> f64 {
        let p = if a <= b { a } else { b };
        let q = if a >= b { a } else { b };
        if p <= 0.0 {
            self.xermsg("DLBETA", "BOTH ARGUMENTS MUST BE GT ZERO");
        }
        if p >= 10.0 {
            let corr = self.d9lgmc(p) + self.d9lgmc(q) - self.d9lgmc(p + q);
            let d1 = -p / (p + q);
            return log(q) * -0.5 + SQ2PIL + corr + (p - 0.5) * log(p / (p + q)) + q * self.dlnrel(d1);
        }
        if q >= 10.0 {
            let corr = self.d9lgmc(q) - self.d9lgmc(p + q);
            let d1 = -p / (p + q);
            return self.dlngam(p) + corr + p - p * log(p + q) + (q - 0.5) * self.dlnrel(d1);
        }
        let gp = self.dgamma(p);
        let gq = self.dgamma(q);
        let gpq = self.dgamma(p + q);
        log(gp * (gq / gpq))
    }

    /// `dbetai`: the incomplete beta function ratio `I_x(p, q)`.
    pub fn dbetai(&mut self, x: f64, pin: f64, qin: f64) -> f64 {
        let eps = d1mach(3);
        let alneps = log(eps);
        let sml = d1mach(1);
        let alnsml = log(sml);
        if !(0.0..=1.0).contains(&x) {
            self.xermsg("DBETAI", "X IS NOT IN THE RANGE (0,1)");
        }
        if pin <= 0.0 || qin <= 0.0 {
            self.xermsg("DBETAI", "P AND/OR Q IS LE ZERO");
        }
        let mut y = x;
        let mut p = pin;
        let mut q = qin;
        if !(q <= p && x < 0.8 || x < 0.2) {
            y = 1.0 - y;
            p = qin;
            q = pin;
        }
        let flip = |r: f64, y: f64, p: f64| if y != x || p != pin { 1.0 - r } else { r };
        if (p + q) * y / (p + 1.0) < eps {
            // L80
            let mut r = 0.0;
            let xb = p * log(if y >= sml { y } else { sml }) - log(p) - self.dlbeta(p, q);
            if xb > alnsml && y != 0.0 {
                r = exp(xb);
            }
            return flip(r, y, p);
        }
        let mut ps = q - d_int(q);
        if ps == 0.0 {
            ps = 1.0;
        }
        let xb = p * log(y) - self.dlbeta(ps, p) - log(p);
        let mut r = 0.0;
        if xb >= alnsml {
            r = exp(xb);
            let mut term = r * p;
            if ps != 1.0 {
                let d1 = alneps / log(y);
                let n = (if d1 >= 4.0 { d1 } else { 4.0 }) as i64;
                for i in 1..=n {
                    let xi = i as f64;
                    term = term * (xi - ps) * y / xi;
                    r += term / (p + xi);
                }
            }
        }
        // L40
        if q > 1.0 {
            let xb = p * log(y) + q * log(1.0 - y) - self.dlbeta(p, q) - log(q);
            let d1 = xb / alnsml;
            let mut ib = (if d1 >= 0.0 { d1 } else { 0.0 }) as i64;
            let mut term = exp(xb - ib as f64 * alnsml);
            let c = 1.0 / (1.0 - y);
            let p1 = q * c / (p + q - 1.0);
            let mut finsum = 0.0;
            let mut n = q as i64;
            if q == n as f64 {
                n -= 1;
            }
            for i in 1..=n {
                if p1 <= 1.0 && term / eps <= finsum {
                    break;
                }
                let xi = i as f64;
                term = (q - xi + 1.0) * c * term / (p + q - xi);
                if term > 1.0 {
                    ib -= 1;
                }
                if term > 1.0 {
                    term *= sml;
                }
                if ib == 0 {
                    finsum += term;
                }
            }
            r += finsum;
        }
        // L70
        r = flip(r, y, p);
        let d1 = if r <= 1.0 { r } else { 1.0 };
        if d1 >= 0.0 { d1 } else { 0.0 }
    }

    /// `TDistribution::PDF(x, dof)`.
    fn t_pdf(&mut self, x: f64, dof: u64) -> f64 {
        let dof = dof as f64;
        let a = 0.5 * (dof + 1.0);
        let b = 0.5 * dof;
        (self.dgamma(a) / self.dgamma(b))
            / ((dof * std::f64::consts::PI).sqrt()
                * larmorx_core::math::pow(1.0 + (x * x) / dof, a))
    }

    /// `TDistribution::CDF(x, dof)`.
    fn t_cdf(&mut self, x: f64, dof: u64) -> f64 {
        let dof = dof as f64;
        let bx = dof / (dof + (x * x));
        let pin = dof / 2.0;
        let qin = 0.5;
        if x >= 0.0 {
            1.0 - 0.5 * self.dbetai(bx, pin, qin)
        } else {
            0.5 * self.dbetai(bx, pin, qin)
        }
    }

    /// `TDistribution::InverseCDF(p, dof)`: a Cornish-Fisher start from the Gaussian's inverse,
    /// then three Newton steps.
    pub fn t_inverse_cdf(&mut self, p: f64, dof: u64) -> f64 {
        if p <= 0.0 {
            return -f64::MAX;
        } else if p >= 1.0 {
            return f64::MAX;
        }
        let d = dof as f64;
        let d2 = d * d;
        let d3 = d * d2;
        let d4 = d * d3;
        let g = gaussian_inverse_cdf(p);
        let pow = larmorx_core::math::pow;
        let g3 = pow(g, 3.0);
        let g5 = pow(g, 5.0);
        let g7 = pow(g, 7.0);
        let g9 = pow(g, 9.0);
        let mut x = g
            + (g3 + g) / (4.0 * d)
            + (5.0 * g5 + 16.0 * g3 + 3.0 * g) / (96.0 * d2)
            + (3.0 * g7 + 19.0 * g5 + 17.0 * g3 - 15.0 * g) / (384.0 * d3)
            + (79.0 * g9 + 776.0 * g7 + 1482.0 * g5 - 1920.0 * g3 - 945.0 * g) / (92160.0 * d4);
        for _ in 0..3 {
            let delta = (p - self.t_cdf(x, dof)) / self.t_pdf(x, dof);
            x += delta;
        }
        x
    }
}

/// `GaussianDistribution::InverseCDF(p)`: Abramowitz and Stegun 26.2.23, then three Newton
/// steps with `vnl_erfc`.
pub fn gaussian_inverse_cdf(p: f64) -> f64 {
    let dp = if p <= 0.5 { p } else { 1.0 - p };
    if p <= 0.0 {
        return -f64::MAX;
    } else if p >= 1.0 {
        return f64::MAX;
    }
    let dt = (-2.0 * log(dp)).sqrt();
    let mut dx = dt
        - ((0.010328e+0 * dt + 0.802853e+0) * dt + 2.515517e+0)
            / (((0.001308e+0 * dt + 0.189269e+0) * dt + 1.432788e+0) * dt + 1.0e+0);
    for _ in 0..3 {
        let dq = 0.5e+0 * larmorx_interp::vnl::erfc(dx * std::f64::consts::FRAC_1_SQRT_2) - dp;
        let ddq = exp(-0.5e+0 * dx * dx) / 2.506628274631000e+0;
        dx += dq / ddq;
    }
    if p <= 0.5 { -dx } else { dx }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gamma_and_beta() {
        let mut s = Slatec::default();
        assert!((s.dgamma(5.0) - 24.0).abs() < 1e-12);
        assert!((s.dgamma(0.5) - std::f64::consts::PI.sqrt()).abs() < 1e-14);
        assert!((s.dgamma(12.5) - 1.3684336546791e8).abs() / 1.37e8 < 1e-10);
        // I_0.5(2, 2) = 0.5
        assert!((s.dbetai(0.5, 2.0, 2.0) - 0.5).abs() < 1e-14);
        assert!(s.messages.is_empty());
    }

    #[test]
    fn t_quantiles() {
        let mut s = Slatec::default();
        // t_{0.975, 10} = 2.228138851986...
        assert!((s.t_inverse_cdf(0.975, 10) - 2.228138851986).abs() < 1e-9);
        assert!((gaussian_inverse_cdf(0.975) - 1.959963984540054).abs() < 1e-9);
    }
}
