// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mris_calc`'s operations (`mris_calc.cpp` 62-638): the functions of one or two `float`s that
//! return a `double` (`fn_mul` ... `fn_dev`), with their `static` running values in [`Statics`],
//! and the operation names (`operation_lookup`, 1627-1824).
//!
//! Each function follows the official build's code (GCC 8.4.0 at `-O3`, SSE2; its disassembly):
//! the arithmetic in `float` where C's operands are `float` (`fn_mul` is `mulss`, `fn_pctdiff`
//! computes `(a - b) * 100 / ((a + b + FLT_MIN) * 0.5)`), in `double` where they are (`fn_inv`,
//! `fn_sig2p`, `fn_bcor`, the sums); the C library's single-precision functions where C++ picks
//! them for `float` arguments (`fmodf`, `powf`, `atan2f`, `log10f`, `logf`, `sqrtss`), `pow` and
//! `log10` in double precision. NaN bits are SSE's ([`larmorx_freesurfer_core::sse`]: a NaN made
//! from numbers is the default NaN `0xffc00000`, a NaN operand passed on quieted, of two the first
//! operand GCC put in the destination register), and the `float` result goes through `cvtss2sd`
//! (a signalling NaN quieted) before the caller stores it with `cvtsd2ss` ([`to_f64`],
//! [`to_f32`]). The C library's results: [`Lib`] (larmorx's correctly rounded functions by
//! default, glibc 2.35's with `reproduce`, GLIBC-07; their NaNs are glibc's in both modes).

use larmorx_core::math::{self, Libm};
use larmorx_freesurfer_core::sse::{
    DEFAULT_NAN32, add, add64, div, div64, mul, mul64, sqrt, sub, sub64,
};

/// `FLT_MIN`.
const FLT_MIN: f32 = f32::MIN_POSITIVE;
/// `2.0 * FLT_MIN`, which GCC compares as a `float` (exactly 2^-125).
const TWO_FLT_MIN: f32 = 2.0 * f32::MIN_POSITIVE;

/// The operations `operation_lookup` knows (`e_operation`), in its order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Mul,
    Div,
    Mod,
    Add,
    Pow,
    Sub,
    Sub0,
    Sratio,
    Pctdiff,
    Pctdiff0,
    Sqd,
    Sqr,
    Not,
    Sqrt,
    Set,
    Atan2,
    Bcor,
    Sig2p,
    Mag,
    Log10,
    Abs,
    Inv,
    Sign,
    Eq,
    Lt,
    Lte,
    Gt,
    Gte,
    Masked,
    And,
    Or,
    Andbw,
    Orbw,
    Upperlimit,
    Lowerlimit,
    Ascii,
    Stats,
    Min,
    Max,
    Size,
    Mean,
    Std,
    Sum,
    Prod,
    Norm,
    Log,
    Unknown,
}

/// `operation_lookup(name)`: exact, case-sensitive names (`mini` and `maxi` are none: the program
/// declares those operations but never looks them up).
pub fn operation_lookup(name: &str) -> Op {
    match name {
        "mul" => Op::Mul,
        "div" => Op::Div,
        "mod" => Op::Mod,
        "add" => Op::Add,
        "pow" => Op::Pow,
        "sub" => Op::Sub,
        "sub0" => Op::Sub0,
        "sratio" => Op::Sratio,
        "pctdiff" => Op::Pctdiff,
        "pctdiff0" => Op::Pctdiff0,
        "sqd" => Op::Sqd,
        "sqr" => Op::Sqr,
        "sqrt" => Op::Sqrt,
        "not" => Op::Not,
        "set" => Op::Set,
        "atan2" => Op::Atan2,
        "bcor" => Op::Bcor,
        "sig2p" => Op::Sig2p,
        "mag" => Op::Mag,
        "log10" => Op::Log10,
        "abs" => Op::Abs,
        "inv" => Op::Inv,
        "sign" => Op::Sign,
        "eq" => Op::Eq,
        "lt" => Op::Lt,
        "lte" => Op::Lte,
        "gt" => Op::Gt,
        "gte" => Op::Gte,
        "masked" => Op::Masked,
        "and" => Op::And,
        "or" => Op::Or,
        "andbw" => Op::Andbw,
        "orbw" => Op::Orbw,
        "upperlimit" => Op::Upperlimit,
        "lowerlimit" => Op::Lowerlimit,
        "stats" => Op::Stats,
        "size" => Op::Size,
        "min" => Op::Min,
        "max" => Op::Max,
        "mean" => Op::Mean,
        "std" => Op::Std,
        "sum" => Op::Sum,
        "prod" => Op::Prod,
        "norm" => Op::Norm,
        "log" => Op::Log,
        "ascii" => Op::Ascii,
        _ => Op::Unknown,
    }
}

/// `b_outCurvFile_write(op)`: whether the operation writes an output file.
pub fn writes_output(op: Op) -> bool {
    !matches!(
        op,
        Op::Ascii
            | Op::Stats
            | Op::Min
            | Op::Max
            | Op::Size
            | Op::Mean
            | Op::Std
            | Op::Sum
            | Op::Prod
            | Op::Unknown
    )
}

/// The operations of two values (`CURV_functionRunABC`'s); the others take one.
pub fn is_binary(op: Op) -> bool {
    matches!(
        op,
        Op::Mul
            | Op::Div
            | Op::Mod
            | Op::Pow
            | Op::Add
            | Op::Sub
            | Op::Sub0
            | Op::Sratio
            | Op::Pctdiff
            | Op::Pctdiff0
            | Op::Set
            | Op::Atan2
            | Op::Bcor
            | Op::Mag
            | Op::Sqd
            | Op::Eq
            | Op::Lt
            | Op::Lte
            | Op::Gt
            | Op::Gte
            | Op::Masked
            | Op::And
            | Op::Or
            | Op::Andbw
            | Op::Orbw
            | Op::Upperlimit
            | Op::Lowerlimit
    )
}

/// `cvtss2sd`: a `float` widened; a signalling NaN quieted (sign and payload kept), with explicit
/// bits so that no build folds the conversion away.
#[inline(always)]
pub fn to_f64(x: f32) -> f64 {
    if x.is_nan() {
        let b = x.to_bits() | 0x0040_0000;
        let sign = u64::from(b >> 31) << 63;
        f64::from_bits(sign | 0x7ff0_0000_0000_0000 | (u64::from(b & 0x007f_ffff) << 29))
    } else {
        f64::from(x)
    }
}

/// `cvtsd2ss`: a `double` rounded to `float`; a NaN keeps its sign and the top of its payload,
/// quieted.
#[inline(always)]
pub fn to_f32(x: f64) -> f32 {
    if x.is_nan() {
        let b = x.to_bits() | 0x0008_0000_0000_0000;
        let sign = ((b >> 63) as u32) << 31;
        f32::from_bits(sign | 0x7f80_0000 | ((b >> 29) as u32 & 0x007f_ffff))
    } else {
        x as f32
    }
}

/// A NaN quieted (`cvtss2sd` then `cvtsd2ss`), every other value as it is.
#[inline(always)]
fn quiet(x: f32) -> f32 {
    if x.is_nan() {
        f32::from_bits(x.to_bits() | 0x0040_0000)
    } else {
        x
    }
}

/// glibc 2.35's `fmodf(x, y)` (measured): a NaN `x` quieted, else a NaN `y` quieted, else the
/// default NaN for an infinite `x` or a zero `y`, else the exact remainder (`x` for an infinite
/// `y`).
pub fn fmodf_glibc(x: f32, y: f32) -> f32 {
    if x.is_nan() {
        quiet(x)
    } else if y.is_nan() {
        quiet(y)
    } else if x.is_infinite() || y == 0.0 {
        DEFAULT_NAN32
    } else if y.is_infinite() {
        x
    } else {
        x % y
    }
}

/// The C library functions of one run: [`Libm`]'s choice (correctly rounded by default, glibc's
/// with `reproduce`) and its witness, a NaN result always glibc's bits (larmorx's correctly
/// rounded functions give a canonical NaN; the official build's NaNs are glibc's).
pub struct Lib {
    pub libm: Libm,
}

impl Lib {
    fn pick32(&self, cr: impl Fn() -> f32, gl: impl Fn() -> f32) -> f32 {
        let libm = &self.libm;
        if libm.is_glibc() {
            let v = gl();
            if libm.watching() {
                let c = cr();
                libm.note(!(c.is_nan() && v.is_nan()) && c.to_bits() != v.to_bits());
            }
            return v;
        }
        let c = cr();
        if c.is_nan() {
            let v = gl();
            if v.is_nan() {
                return v;
            }
        }
        if libm.watching() {
            let v = gl();
            libm.note(!(c.is_nan() && v.is_nan()) && c.to_bits() != v.to_bits());
        }
        c
    }

    fn pick64(&self, cr: impl Fn() -> f64, gl: impl Fn() -> f64) -> f64 {
        let libm = &self.libm;
        if libm.is_glibc() {
            let v = gl();
            if libm.watching() {
                let c = cr();
                libm.note(!(c.is_nan() && v.is_nan()) && c.to_bits() != v.to_bits());
            }
            return v;
        }
        let c = cr();
        if c.is_nan() {
            let v = gl();
            if v.is_nan() {
                return v;
            }
        }
        if libm.watching() {
            let v = gl();
            libm.note(!(c.is_nan() && v.is_nan()) && c.to_bits() != v.to_bits());
        }
        c
    }

    /// `powf(x, y)`.
    pub fn powf(&self, x: f32, y: f32) -> f32 {
        self.pick32(|| math::powf(x, y), || math::powf_glibc(x, y))
    }

    /// `atan2f(y, x)` (by default the correctly rounded double rounded to `float`, as
    /// [`Libm::atan2f`]).
    pub fn atan2f(&self, y: f32, x: f32) -> f32 {
        self.pick32(
            || math::atan2(f64::from(y), f64::from(x)) as f32,
            || math::atan2f_glibc(y, x),
        )
    }

    /// `log10f(x)`.
    pub fn log10f(&self, x: f32) -> f32 {
        self.pick32(|| math::log10f(x), || math::log10f_glibc(x))
    }

    /// `logf(x)`.
    pub fn logf(&self, x: f32) -> f32 {
        self.pick32(|| math::logf(x), || math::logf_glibc(x))
    }

    /// `pow(x, y)`.
    pub fn pow(&self, x: f64, y: f64) -> f64 {
        self.pick64(|| math::pow(x, y), || math::pow_glibc(x, y))
    }

    /// `log10(x)`.
    pub fn log10(&self, x: f64) -> f64 {
        self.pick64(|| math::log10(x), || math::log10_glibc(x))
    }
}

/// The C library functions an operation calls (GLIBC-07's text), `None` for those that call
/// none whose bits can differ.
pub fn libm_names(op: Op) -> Option<&'static str> {
    match op {
        Op::Pow => Some("powf"),
        Op::Atan2 => Some("atan2f"),
        Op::Log10 => Some("log10f"),
        Op::Log => Some("logf"),
        Op::Bcor => Some("pow and log10"),
        Op::Sig2p => Some("pow"),
        _ => None,
    }
}

/// `-|a|` as GCC computes it (`andps`, `xorps`: the bits only), widened.
fn neg_abs(a: f32) -> f64 {
    to_f64(f32::from_bits((a.to_bits() & 0x7fff_ffff) ^ 0x8000_0000))
}

/// `fn_sign(a)`: -1, 0 or 1 (0 for a NaN).
pub fn sign(a: f32) -> f64 {
    let mut r = 0.0;
    if 0.0 > a {
        r = -1.0;
    }
    if a > 0.0 {
        r = 1.0;
    }
    r
}

/// The operation of two values at one index (`CURV_functionRunABC`'s `F`), as `double`.
/// `andbw` and `orbw` convert with `to_int` (`cvttss2si`).
pub fn binary(op: Op, a: f32, b: f32, lib: &Lib, to_int: &mut dyn FnMut(f32) -> i32) -> f64 {
    match op {
        Op::Mul => to_f64(mul(a, b)),
        Op::Div => {
            if b == 0.0 {
                0.0
            } else {
                to_f64(div(a, b))
            }
        }
        Op::Mod => to_f64(fmodf_glibc(a, b)),
        Op::Add => to_f64(add(a, b)),
        Op::Pow => to_f64(lib.powf(a, b)),
        Op::Sub => to_f64(sub(a, b)),
        Op::Sub0 => {
            if TWO_FLT_MIN > a.abs() || TWO_FLT_MIN > b.abs() {
                0.0
            } else {
                to_f64(sub(a, b))
            }
        }
        Op::Sratio => {
            if TWO_FLT_MIN > a.abs() || TWO_FLT_MIN > b.abs() {
                0.0
            } else if a > b {
                to_f64(div(a, add(b, FLT_MIN)))
            } else {
                to_f64(div(-b, add(a, FLT_MIN)))
            }
        }
        Op::Pctdiff => to_f64(pctdiff(a, b)),
        Op::Pctdiff0 => {
            if TWO_FLT_MIN > a.abs() || TWO_FLT_MIN > b.abs() {
                0.0
            } else {
                to_f64(pctdiff(a, b))
            }
        }
        Op::Sqd => {
            let d = sub(a, b);
            to_f64(mul(d, d))
        }
        Op::Set => to_f64(b),
        Op::Atan2 => to_f64(lib.atan2f(a, b)),
        Op::Bcor => {
            // bonferroni correction of a sig (-log10(p)) value
            let p = lib.pow(10.0, neg_abs(a));
            let p = sub64(1.0, lib.pow(sub64(1.0, p), to_f64(b)));
            let l = lib.log10(p);
            mul64(sign(a), -l)
        }
        Op::Mag => to_f64(sqrt(add(mul(a, a), mul(b, b)))),
        Op::Eq => {
            if a == b {
                to_f64(a)
            } else {
                0.0
            }
        }
        Op::Lt => {
            if a < b {
                to_f64(a)
            } else {
                0.0
            }
        }
        Op::Lte => {
            if a <= b {
                to_f64(a)
            } else {
                0.0
            }
        }
        Op::Gt => {
            if a > b {
                to_f64(a)
            } else {
                0.0
            }
        }
        Op::Gte => {
            if a >= b {
                to_f64(a)
            } else {
                0.0
            }
        }
        Op::Upperlimit => to_f64(if b <= a { b } else { a }),
        Op::Lowerlimit => to_f64(if a <= b { b } else { a }),
        Op::And => f64::from(u8::from(a != 0.0 && b != 0.0)),
        Op::Or => f64::from(u8::from(a != 0.0 || b != 0.0)),
        Op::Andbw => f64::from(to_int(a) & to_int(b)),
        Op::Orbw => f64::from(to_int(a) | to_int(b)),
        Op::Masked => to_f64(if b == 0.0 { b } else { a }),
        _ => unreachable!("not an operation of two values"),
    }
}

/// `fn_pctdiff(a, b)`: `(a - b) * 100 / ((a + b + FLT_MIN) * 0.5)` in `float`.
fn pctdiff(a: f32, b: f32) -> f32 {
    let den = mul(add(add(a, b), FLT_MIN), 0.5);
    div(mul(sub(a, b), 100.0), den)
}

/// The operation of one value (`CURV_functionRunAC`'s `F` for the voxelwise operations).
pub fn unary(op: Op, a: f32, lib: &Lib) -> f64 {
    match op {
        Op::Sig2p => lib.pow(10.0, neg_abs(a)),
        Op::Abs => to_f64(f32::from_bits(a.to_bits() & 0x7fff_ffff)),
        Op::Log10 => {
            if 0.0 >= a {
                0.0
            } else {
                to_f64(lib.log10f(a))
            }
        }
        Op::Log => {
            if 0.0 >= a {
                0.0
            } else {
                to_f64(lib.logf(a))
            }
        }
        Op::Inv => div64(1.0, to_f64(a)),
        Op::Sign => sign(a),
        Op::Sqr => to_f64(mul(a, a)),
        Op::Sqrt => to_f64(sqrt(a)),
        Op::Not => {
            if a > 0.5 {
                0.0
            } else {
                1.0
            }
        }
        _ => unreachable!("not an operation of one value"),
    }
}

/// The `static` running values of the statistics' functions (`fn_min` ... `fn_dev`, `Gfn_sum`,
/// `Gfn_prod`, the counts), as one run of the program leaves them.
#[derive(Debug)]
pub struct Statics {
    min: (f32, i32),
    mini: (f32, i32, i32),
    max: (f32, i32),
    maxi: (f32, i32, i32),
    sum: f64,
    mean_count: i32,
    sum2_dev: f64,
    sum2: f64,
    dev_count: i32,
    zero: i32,
    neg: i32,
    pos: i32,
    /// `Gf_sum`, `Gf_prod`: global, zeroed by the caller.
    pub gsum: f64,
    pub gprod: f64,
}

impl Default for Statics {
    fn default() -> Self {
        Statics {
            min: (0.0, 0),
            mini: (0.0, -1, 0),
            max: (0.0, 0),
            maxi: (0.0, -1, 0),
            sum: 0.0,
            mean_count: 0,
            sum2_dev: 0.0,
            sum2: 0.0,
            dev_count: 1,
            zero: 0,
            neg: 0,
            pos: 0,
            gsum: 0.0,
            gprod: 1.0,
        }
    }
}

/// The running functions `CURV_functionRunAC` takes for the statistics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Run {
    Min,
    Mini,
    Max,
    Maxi,
    Mean,
    Dev,
    GSum,
    GProd,
    NegCount,
    ZeroCount,
    PosCount,
}

impl Statics {
    /// One call of the running function `f` on `a`: its `double` result.
    pub fn call(&mut self, f: Run, a: f32) -> f64 {
        match f {
            Run::Min => {
                let (m, c) = &mut self.min;
                if *c == 0 {
                    *m = a;
                }
                if a <= *m {
                    *m = a;
                }
                *c += 1;
                to_f64(*m)
            }
            Run::Mini => {
                let (m, i, c) = &mut self.mini;
                if *c == 0 {
                    *m = a;
                }
                if a <= *m {
                    *m = a;
                    *i = *c;
                }
                *c += 1;
                // FreeSurfer returns the index as a float (FS-1126: the caller rounds it)
                f64::from(*i)
            }
            Run::Max => {
                let (m, c) = &mut self.max;
                if *c == 0 {
                    *m = a;
                }
                if a >= *m {
                    *m = a;
                }
                *c += 1;
                to_f64(*m)
            }
            Run::Maxi => {
                let (m, i, c) = &mut self.maxi;
                if *c == 0 {
                    *m = a;
                }
                if a >= *m {
                    *m = a;
                    *i = *c;
                }
                *c += 1;
                f64::from(*i)
            }
            Run::Mean => {
                // fn_sum's double sum (a + sum), kept in a float, over the count as a float
                self.sum = add64(to_f64(a), self.sum);
                self.mean_count += 1;
                to_f64(div(to_f32(self.sum), self.mean_count as f32))
            }
            Run::Dev => {
                // fn2_sum and fn_sum2 (a * a in float), then (sum2 * n - sum * sum) / n
                self.sum2_dev = add64(to_f64(a), self.sum2_dev);
                self.sum2 = add64(to_f64(mul(a, a)), self.sum2);
                let n = f64::from(self.dev_count);
                self.dev_count += 1;
                let s = self.sum2_dev;
                div64(sub64(mul64(self.sum2, n), mul64(s, s)), n)
            }
            Run::GSum => {
                self.gsum = add64(to_f64(a), self.gsum);
                self.gsum
            }
            Run::GProd => {
                self.gprod = mul64(to_f64(a), self.gprod);
                self.gprod
            }
            Run::ZeroCount => {
                if a == 0.0 {
                    self.zero += 1;
                }
                f64::from(self.zero)
            }
            Run::NegCount => {
                if 0.0 > a {
                    self.neg += 1;
                }
                f64::from(self.neg)
            }
            Run::PosCount => {
                if a > 0.0 {
                    self.pos += 1;
                }
                f64::from(self.pos)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    fn lib(mode: UpstreamDefects) -> Lib {
        Lib {
            libm: Libm::new(mode),
        }
    }

    /// The official program's results on NaN, infinite and denormal operands
    /// (`validation/investigations/freesurfer-mris-calc-probe`'s curvature cases, observed).
    #[test]
    fn nans_as_the_official_build() {
        let f = |b: u32| f32::from_bits(b);
        let c = [
            1.0f32,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f(0x7fa0_0007),
            5.0,
            0.0,
            -3.0,
            f(1),
        ];
        let d = [
            f(0x7fa0_0009),
            f(0x7fc0_000b),
            f(0x7fa0_000c),
            f(0x7fc0_000d),
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            0.0,
        ];
        let want: [(Op, [u32; 8]); 12] = [
            (
                Op::Mod,
                [
                    0x7fe00009, 0x7fc0000b, 0x7fe0000c, 0x7fe00007, 0x40a00000, 0, 0xc0400000,
                    0xffc00000,
                ],
            ),
            (
                Op::Pow,
                [
                    0x7fe00009, 0x7fc0000b, 0x7fe0000c, 0x7fe00007, 0x7f800000, 0, 0, 0x3f800000,
                ],
            ),
            (
                Op::Atan2,
                [
                    0x7fe00009, 0x7fc0000b, 0x7fe0000c, 0x7fc0000d, 0, 0, 0xc0490fdb, 0x3fc90fdb,
                ],
            ),
            (
                Op::Sub0,
                [
                    0x7fe00009, 0x7fc0000b, 0x7fe0000c, 0x7fe00007, 0xff800000, 0, 0x7f800000, 0,
                ],
            ),
            (
                Op::Sratio,
                [
                    0xffe00009, 0xffc0000b, 0xffe0000c, 0xffc0000d, 0xff800000, 0, 0, 0,
                ],
            ),
            (
                Op::Pctdiff,
                [
                    0x7fe00009, 0x7fc0000b, 0x7fe0000c, 0x7fe00007, 0xffc00000, 0xffc00000,
                    0xffc00000, 0x37c80000,
                ],
            ),
            (
                Op::Mag,
                [
                    0x7fe00009, 0x7fc0000b, 0x7fe0000c, 0x7fe00007, 0x7f800000, 0x7f800000,
                    0x7f800000, 0,
                ],
            ),
            (
                Op::Masked,
                [
                    0x3f800000, 0x7f800000, 0xff800000, 0x7fe00007, 0x40a00000, 0, 0xc0400000, 0,
                ],
            ),
            (
                Op::Upperlimit,
                [
                    0x3f800000, 0x7f800000, 0xff800000, 0x7fe00007, 0x40a00000, 0, 0xff800000, 0,
                ],
            ),
            (
                Op::Lowerlimit,
                [
                    0x3f800000, 0x7f800000, 0xff800000, 0x7fe00007, 0x7f800000, 0x7f800000,
                    0xc0400000, 1,
                ],
            ),
            (
                Op::Orbw,
                [
                    0xcf000000, 0xcf000000, 0xcf000000, 0xcf000000, 0xcf000000, 0xcf000000,
                    0xc0400000, 0,
                ],
            ),
            (
                Op::Bcor,
                [
                    0xffe00009, 0x7f800000, 0xff800000, 0x7fe00007, 0x80000000, 0x80000000,
                    0xffc00000, 0x7f800000,
                ],
            ),
        ];
        // x86-64's cvttss2si
        let mut x86 = |v: f32| {
            if v.is_nan() || v >= 2147483648.0 || v < -2147483648.0 {
                i32::MIN
            } else {
                v as i32
            }
        };
        for mode in [UpstreamDefects::Reproduce, UpstreamDefects::Fix] {
            let l = lib(mode);
            for (op, w) in want {
                for i in 0..8 {
                    let got = to_f32(binary(op, c[i], d[i], &l, &mut x86)).to_bits();
                    assert_eq!(got, w[i], "{op:?} {i} {mode:?}");
                }
            }
        }
    }

    #[test]
    fn conversions_keep_nan_bits() {
        let s = f32::from_bits(0x7fa0_0001);
        assert_eq!(to_f32(to_f64(s)).to_bits(), 0x7fe0_0001);
        let n = f32::from_bits(0xffc0_0002);
        assert_eq!(to_f32(to_f64(n)).to_bits(), 0xffc0_0002);
        assert_eq!(to_f32(0.1f64), 0.1f32);
    }
}
