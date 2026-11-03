// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-cprob 2111.0 (drand.cc, ndtri.cc, polevl.cc,
// mtherr.cc, const.cc), FSL's copy of Stephen L. Moshier's Cephes Math Library (Copyright 1984,
// 1987, 1989, 1995 by Stephen L. Moshier), compiled with `UNK` (the decimal constants, no bit
// patterns). Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the originals.
// Distributed under the FSL Licence (see LICENSE).
//! FSL's `cprob` library: the probability functions fslmaths calls.
//!
//! - [`Drand`]: `drand` and `sdrand`, the three-generator random numbers of Wichmann and Hill
//!   (BYTE, March 1987), uniform in `[1, 2)`; the state starts at `(1, 10000, 3000)` in every
//!   process.
//! - [`ndtri`]: the inverse of the standard normal distribution function, with Cephes' error
//!   report ([`mtherr`]: printed on standard output) outside `(0, 1)`.

// Cephes' decimal constants are kept as written (the compiler rounds them as FSL's did).
#![allow(clippy::excessive_precision, clippy::unreadable_literal)]

use crate::glibc_double;

/// Cephes' `MAXNUM` (2^1024 (1 - MACHEP)).
pub const MAXNUM: f64 = 1.797_693_134_862_315_708_15E308;

/// The state of `drand` (`sx`, `sy`, `sz`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Drand {
    sx: i32,
    sy: i32,
    sz: i32,
}

impl Default for Drand {
    fn default() -> Self {
        Drand {
            sx: 1,
            sy: 10000,
            sz: 3000,
        }
    }
}

impl Drand {
    /// `ranwh()`: the three congruential generators.
    fn ranwh(&mut self) {
        // sx = sx * 171 mod 30269
        let r = self.sx / 177;
        let s = self.sx - 177 * r;
        self.sx = 171 * s - 2 * r;
        if self.sx < 0 {
            self.sx += 30269;
        }
        // sy = sy * 172 mod 30307
        let r = self.sy / 176;
        let s = self.sy - 176 * r;
        self.sy = 172 * s - 35 * r;
        if self.sy < 0 {
            self.sy += 30307;
        }
        // sz = 170 * sz mod 30323
        let r = self.sz / 178;
        let s = self.sz - 178 * r;
        self.sz = 170 * s - 63 * r;
        if self.sz < 0 {
            self.sz += 30323;
        }
    }

    /// `drand(&x)`: a number in `[1, 2)` (`UNK`: the fraction of the three generators' sum,
    /// plus 1).
    pub fn drand(&mut self) -> f64 {
        self.ranwh();
        let mut d = f64::from(self.sx) / 30269.0
            + f64::from(self.sy) / 30307.0
            + f64::from(self.sz) / 30323.0;
        // unsigned short r = d (the sum is below 3: no overflow)
        let r = d as u16;
        d -= f64::from(r);
        d + 1.0
    }

    /// `sdrand(seed1, seed2, seed3)`: `seed % 30000 + 1` each (C's remainder: a negative seed
    /// gives a negative state).
    pub fn sdrand(&mut self, seed1: i32, seed2: i32, seed3: i32) {
        self.sx = seed1 % 30000 + 1;
        self.sy = seed2 % 30000 + 1;
        self.sz = seed3 % 30000 + 1;
    }
}

/// Cephes' `mtherr(name, code)` with its default handler: what it prints on standard output.
pub fn mtherr(name: &str, code: i32) -> String {
    const ERMSG: [&str; 7] = [
        "unknown",
        "domain",
        "singularity",
        "overflow",
        "underflow",
        "total loss of precision",
        "partial loss of precision",
    ];
    let c = if (1..7).contains(&code) { code } else { 0 };
    format!("\n{name} {} error\n", ERMSG[c as usize])
}

/// Cephes' error code `DOMAIN`.
pub const DOMAIN: i32 = 1;

/// `polevl(x, coef, N)`: `coef[0] x^N + ... + coef[N]` by Horner's rule.
fn polevl(x: f64, coef: &[f64]) -> f64 {
    let mut ans = coef[0];
    for &c in &coef[1..] {
        ans = ans * x + c;
    }
    ans
}

/// `p1evl(x, coef, N)`: the same with a leading coefficient 1 (not stored).
fn p1evl(x: f64, coef: &[f64]) -> f64 {
    let mut ans = x + coef[0];
    for &c in &coef[1..] {
        ans = ans * x + c;
    }
    ans
}

const S2PI: f64 = 2.506_628_274_631_000_502_42E0;

const P0: [f64; 5] = [
    -5.996_335_010_141_078_952_67E1,
    9.800_107_541_859_996_615_36E1,
    -5.667_628_574_690_702_934_39E1,
    1.393_126_093_872_796_795_03E1,
    -1.239_165_838_673_812_580_16E0,
];
const Q0: [f64; 8] = [
    1.954_488_583_381_417_598_34E0,
    4.676_279_128_988_815_384_53E0,
    8.636_024_213_908_905_905_75E1,
    -2.254_626_878_541_193_705_27E2,
    2.002_602_123_800_606_603_59E2,
    -8.203_722_561_683_333_399_12E1,
    1.590_562_251_262_116_955_15E1,
    -1.183_316_211_213_300_031_42E0,
];
const P1: [f64; 9] = [
    4.055_448_923_059_624_199_23E0,
    3.152_510_945_998_938_661_54E1,
    5.716_281_922_464_212_881_62E1,
    4.408_050_738_932_008_347_00E1,
    1.468_495_619_288_580_240_14E1,
    2.186_633_068_507_902_675_39E0,
    -1.402_560_791_713_544_958_75E-1,
    -3.504_246_268_278_482_034_18E-2,
    -8.574_567_851_546_854_136_11E-4,
];
const Q1: [f64; 8] = [
    1.577_998_832_564_667_497_31E1,
    4.539_076_351_288_792_105_84E1,
    4.131_720_382_546_720_304_40E1,
    1.504_253_856_929_075_034_08E1,
    2.504_649_462_083_094_159_79E0,
    -1.421_829_228_547_877_885_74E-1,
    -3.808_064_076_915_782_771_94E-2,
    -9.332_594_808_954_574_273_72E-4,
];
const P2: [f64; 9] = [
    3.237_748_917_769_460_359_70E0,
    6.915_228_890_689_842_116_95E0,
    3.938_810_252_924_744_434_15E0,
    1.333_034_608_158_075_423_89E0,
    2.014_853_895_491_790_815_38E-1,
    1.237_166_348_178_200_213_58E-2,
    3.015_815_535_082_354_160_07E-4,
    2.658_069_746_867_375_508_32E-6,
    6.239_745_391_849_832_937_30E-9,
];
const Q2: [f64; 8] = [
    6.024_270_393_647_420_142_55E0,
    3.679_835_638_561_604_594_03E0,
    1.377_020_994_890_813_302_71E0,
    2.162_369_935_944_966_358_90E-1,
    1.342_040_060_885_431_890_37E-2,
    3.280_144_646_821_277_391_04E-4,
    2.892_478_647_453_806_839_36E-6,
    6.790_194_080_099_812_744_25E-9,
];

/// `ndtri(y0)`: the argument at which the standard normal distribution function is `y0`, and
/// what `mtherr` printed (`y0` outside `(0, 1)`: -`MAXNUM` or `MAXNUM`). glibc's `log`; no fused
/// multiply-adds (FSL's build has none).
pub fn ndtri(y0: f64) -> (f64, Option<String>) {
    if y0 <= 0.0 {
        return (-MAXNUM, Some(mtherr("ndtri", DOMAIN)));
    }
    if y0 >= 1.0 {
        return (MAXNUM, Some(mtherr("ndtri", DOMAIN)));
    }
    let mut code = 1;
    let mut y = y0;
    if y > 1.0 - 0.135_335_283_236_612_691_89 {
        y = 1.0 - y;
        code = 0;
    }
    if y > 0.135_335_283_236_612_691_89 {
        y -= 0.5;
        let y2 = y * y;
        let x = y + y * (y2 * polevl(y2, &P0) / p1evl(y2, &Q0));
        return (x * S2PI, None);
    }
    let x = (-2.0 * glibc_double::log(y)).sqrt();
    let x0 = x - glibc_double::log(x) / x;
    let z = 1.0 / x;
    let x1 = if x < 8.0 {
        z * polevl(z, &P1) / p1evl(z, &Q1)
    } else {
        z * polevl(z, &P2) / p1evl(z, &Q2)
    };
    let mut x = x0 - x1;
    if code != 0 {
        x = -x;
    }
    (x, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drand_starts_from_the_fixed_state() {
        let mut d = Drand::default();
        let a = d.drand();
        assert!((1.0..2.0).contains(&a));
        let mut e = Drand::default();
        assert_eq!(a.to_bits(), e.drand().to_bits());
    }

    #[test]
    fn ndtri_values() {
        assert_eq!(ndtri(0.5).0, 0.0);
        assert!((ndtri(0.975).0 - 1.959_963_984_540_054).abs() < 1e-14);
        assert!((ndtri(1e-10).0 + 6.361_340_902_404_056).abs() < 1e-12);
        assert_eq!(ndtri(0.0), (-MAXNUM, Some("\nndtri domain error\n".into())));
    }
}
