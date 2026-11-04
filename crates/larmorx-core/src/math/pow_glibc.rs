// SPDX-License-Identifier: Apache-2.0 AND MIT
// Translated to Rust from Arm's Optimized Routines v19.11 (github.com/ARM-software/optimized-routines,
// tag v19.11): math/pow.c, math/pow_log_data.c (__pow_log_data) and math/math_config.h, with
// math/exp_data.c's table (in `exp_glibc`). Copyright (c) 2018, Arm Limited, under the MIT licence
// (its text and notice are in LICENSES/MIT-arm-optimized-routines.txt and NOTICE).
// Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the originals.
//! The power function for binary64 with the bits of glibc 2.35's `pow` on x86-64 (the FMA build,
//! `__pow_fma`, `sysdeps/ieee754/dbl-64/e_pow.c` compiled with `-mfma -mavx2`), on every
//! platform.
//!
//! glibc 2.35's `pow` is Arm's routine (`POW_LOG_TABLE_BITS` 7, `POW_LOG_POLY_ORDER` 8, every
//! number identical: `validation/investigations/fsl-bet/arm_glibc_pow_exp.py`); this module is
//! translated from Arm's MIT originals, not from glibc. Every fused multiply-add of the FMA build
//! is a `mul_add` here: the three the source asks for (`fma` with `HAVE_FAST_FMA`) and the ones
//! GCC contracted (read from `objdump -d` of Ubuntu 22.04's libm.so.6, glibc 2.35-0ubuntu3),
//! correctly rounded on every platform; the NaN results are SSE's (`super::x86_nan`). Not
//! correctly rounded (Arm: 0.52 ulp; about 0.09 % of calls); [`super::pow`] is, and is larmorx's
//! default. This one serves `reproduce` (FSL's BET: its initial radius `pow(x, 1/3)` and its
//! threshold exponent `pow(f, 0.275)`; `fslmaths -pow`). Until 2026-11-04 the FSL replicas' core
//! carried its own translation (`larmorx-nc-core`'s `glibc_double`); it now calls this one.
//! Proved equal to the system's `pow` (glibc 2.35, x86-64) and to that translation on more than
//! 10^9 pairs, NaN bits included (`validation/investigations/glibc-maths/`).

// The C source's literals and expression forms are kept (`lo = one - hi + y + lo`).
#![allow(clippy::unreadable_literal, clippy::assign_op_pattern)]

use super::exp_glibc::exp_data;
use super::x86_nan::{add_f64, div_f64, invalid_f64, mul_f64};

/// `__pow_log_data` (Arm's `pow_log_data.c`, `POW_LOG_TABLE_BITS` 7, `POW_LOG_POLY_ORDER` 8), as
/// 64-bit patterns.
#[rustfmt::skip]
mod pow_log_data {
    /// `ln2hi`, `ln2lo`.
    pub(super) const LN2HI: u64 = 0x3fe62e42fefa3800;
    pub(super) const LN2LO: u64 = 0x3d2ef35793c76730;
    /// `poly`: `A[0..6]`, scaled as `log_inline` uses them.
    pub(super) const POLY: [u64; 7] = [
        0xbfe0000000000000,
        0xbfe5555555555560,
        0x3fe0000000000006,
        0x3fe999999959554e,
        0xbfe555555529a47a,
        0xbff2495b9b4845e9,
        0x3ff0002b8b263fc3,
    ];
    /// `tab`: `(invc, logc, logctail)` per subinterval (`pad` dropped).
    pub(super) const TAB: [(u64, u64, u64); 128] = [
        (0x3ff6a00000000000, 0xbfd62c82f2b9c800, 0x3cfab42428375680),
        (0x3ff6800000000000, 0xbfd5d1bdbf580800, 0xbd1ca508d8e0f720),
        (0x3ff6600000000000, 0xbfd5767717455800, 0xbd2362a4d5b6506d),
        (0x3ff6400000000000, 0xbfd51aad872df800, 0xbce684e49eb067d5),
        (0x3ff6200000000000, 0xbfd4be5f95777800, 0xbd041b6993293ee0),
        (0x3ff6000000000000, 0xbfd4618bc21c6000, 0x3d13d82f484c84cc),
        (0x3ff5e00000000000, 0xbfd404308686a800, 0x3cdc42f3ed820b3a),
        (0x3ff5c00000000000, 0xbfd3a64c55694800, 0x3d20b1c686519460),
        (0x3ff5a00000000000, 0xbfd347dd9a988000, 0x3d25594dd4c58092),
        (0x3ff5800000000000, 0xbfd2e8e2bae12000, 0x3d267b1e99b72bd8),
        (0x3ff5600000000000, 0xbfd2895a13de8800, 0x3d15ca14b6cfb03f),
        (0x3ff5600000000000, 0xbfd2895a13de8800, 0x3d15ca14b6cfb03f),
        (0x3ff5400000000000, 0xbfd22941fbcf7800, 0xbd165a242853da76),
        (0x3ff5200000000000, 0xbfd1c898c1699800, 0xbd1fafbc68e75404),
        (0x3ff5000000000000, 0xbfd1675cababa800, 0x3d1f1fc63382a8f0),
        (0x3ff4e00000000000, 0xbfd1058bf9ae4800, 0xbd26a8c4fd055a66),
        (0x3ff4c00000000000, 0xbfd0a324e2739000, 0xbd0c6bee7ef4030e),
        (0x3ff4a00000000000, 0xbfd0402594b4d000, 0xbcf036b89ef42d7f),
        (0x3ff4a00000000000, 0xbfd0402594b4d000, 0xbcf036b89ef42d7f),
        (0x3ff4800000000000, 0xbfcfb9186d5e4000, 0x3d0d572aab993c87),
        (0x3ff4600000000000, 0xbfcef0adcbdc6000, 0x3d2b26b79c86af24),
        (0x3ff4400000000000, 0xbfce27076e2af000, 0xbd172f4f543fff10),
        (0x3ff4200000000000, 0xbfcd5c216b4fc000, 0x3d21ba91bbca681b),
        (0x3ff4000000000000, 0xbfcc8ff7c79aa000, 0x3d27794f689f8434),
        (0x3ff4000000000000, 0xbfcc8ff7c79aa000, 0x3d27794f689f8434),
        (0x3ff3e00000000000, 0xbfcbc286742d9000, 0x3d194eb0318bb78f),
        (0x3ff3c00000000000, 0xbfcaf3c94e80c000, 0x3cba4e633fcd9066),
        (0x3ff3a00000000000, 0xbfca23bc1fe2b000, 0xbd258c64dc46c1ea),
        (0x3ff3a00000000000, 0xbfca23bc1fe2b000, 0xbd258c64dc46c1ea),
        (0x3ff3800000000000, 0xbfc9525a9cf45000, 0xbd2ad1d904c1d4e3),
        (0x3ff3600000000000, 0xbfc87fa06520d000, 0x3d2bbdbf7fdbfa09),
        (0x3ff3400000000000, 0xbfc7ab890210e000, 0x3d2bdb9072534a58),
        (0x3ff3400000000000, 0xbfc7ab890210e000, 0x3d2bdb9072534a58),
        (0x3ff3200000000000, 0xbfc6d60fe719d000, 0xbd10e46aa3b2e266),
        (0x3ff3000000000000, 0xbfc5ff3070a79000, 0xbd1e9e439f105039),
        (0x3ff3000000000000, 0xbfc5ff3070a79000, 0xbd1e9e439f105039),
        (0x3ff2e00000000000, 0xbfc526e5e3a1b000, 0xbd20de8b90075b8f),
        (0x3ff2c00000000000, 0xbfc44d2b6ccb8000, 0x3d170cc16135783c),
        (0x3ff2c00000000000, 0xbfc44d2b6ccb8000, 0x3d170cc16135783c),
        (0x3ff2a00000000000, 0xbfc371fc201e9000, 0x3cf178864d27543a),
        (0x3ff2800000000000, 0xbfc29552f81ff000, 0xbd248d301771c408),
        (0x3ff2600000000000, 0xbfc1b72ad52f6000, 0xbd2e80a41811a396),
        (0x3ff2600000000000, 0xbfc1b72ad52f6000, 0xbd2e80a41811a396),
        (0x3ff2400000000000, 0xbfc0d77e7cd09000, 0x3d0a699688e85bf4),
        (0x3ff2400000000000, 0xbfc0d77e7cd09000, 0x3d0a699688e85bf4),
        (0x3ff2200000000000, 0xbfbfec9131dbe000, 0xbd2575545ca333f2),
        (0x3ff2000000000000, 0xbfbe27076e2b0000, 0x3d2a342c2af0003c),
        (0x3ff2000000000000, 0xbfbe27076e2b0000, 0x3d2a342c2af0003c),
        (0x3ff1e00000000000, 0xbfbc5e548f5bc000, 0xbd1d0c57585fbe06),
        (0x3ff1c00000000000, 0xbfba926d3a4ae000, 0x3d253935e85baac8),
        (0x3ff1c00000000000, 0xbfba926d3a4ae000, 0x3d253935e85baac8),
        (0x3ff1a00000000000, 0xbfb8c345d631a000, 0x3d137c294d2f5668),
        (0x3ff1a00000000000, 0xbfb8c345d631a000, 0x3d137c294d2f5668),
        (0x3ff1800000000000, 0xbfb6f0d28ae56000, 0xbd269737c93373da),
        (0x3ff1600000000000, 0xbfb51b073f062000, 0x3d1f025b61c65e57),
        (0x3ff1600000000000, 0xbfb51b073f062000, 0x3d1f025b61c65e57),
        (0x3ff1400000000000, 0xbfb341d7961be000, 0x3d2c5edaccf913df),
        (0x3ff1400000000000, 0xbfb341d7961be000, 0x3d2c5edaccf913df),
        (0x3ff1200000000000, 0xbfb16536eea38000, 0x3d147c5e768fa309),
        (0x3ff1000000000000, 0xbfaf0a30c0118000, 0x3d2d599e83368e91),
        (0x3ff1000000000000, 0xbfaf0a30c0118000, 0x3d2d599e83368e91),
        (0x3ff0e00000000000, 0xbfab42dd71198000, 0x3d1c827ae5d6704c),
        (0x3ff0e00000000000, 0xbfab42dd71198000, 0x3d1c827ae5d6704c),
        (0x3ff0c00000000000, 0xbfa77458f632c000, 0xbd2cfc4634f2a1ee),
        (0x3ff0c00000000000, 0xbfa77458f632c000, 0xbd2cfc4634f2a1ee),
        (0x3ff0a00000000000, 0xbfa39e87b9fec000, 0x3cf502b7f526feaa),
        (0x3ff0a00000000000, 0xbfa39e87b9fec000, 0x3cf502b7f526feaa),
        (0x3ff0800000000000, 0xbf9f829b0e780000, 0xbd2980267c7e09e4),
        (0x3ff0800000000000, 0xbf9f829b0e780000, 0xbd2980267c7e09e4),
        (0x3ff0600000000000, 0xbf97b91b07d58000, 0xbd288d5493faa639),
        (0x3ff0400000000000, 0xbf8fc0a8b0fc0000, 0xbcdf1e7cf6d3a69c),
        (0x3ff0400000000000, 0xbf8fc0a8b0fc0000, 0xbcdf1e7cf6d3a69c),
        (0x3ff0200000000000, 0xbf7fe02a6b100000, 0xbd19e23f0dda40e4),
        (0x3ff0200000000000, 0xbf7fe02a6b100000, 0xbd19e23f0dda40e4),
        (0x3ff0000000000000, 0x0000000000000000, 0x0000000000000000),
        (0x3ff0000000000000, 0x0000000000000000, 0x0000000000000000),
        (0x3fefc00000000000, 0x3f80101575890000, 0xbd10c76b999d2be8),
        (0x3fef800000000000, 0x3f90205658938000, 0xbd23dc5b06e2f7d2),
        (0x3fef400000000000, 0x3f98492528c90000, 0xbd2aa0ba325a0c34),
        (0x3fef000000000000, 0x3fa0415d89e74000, 0x3d0111c05cf1d753),
        (0x3feec00000000000, 0x3fa466aed42e0000, 0xbd2c167375bdfd28),
        (0x3fee800000000000, 0x3fa894aa149fc000, 0xbd197995d05a267d),
        (0x3fee400000000000, 0x3faccb73cdddc000, 0xbd1a68f247d82807),
        (0x3fee200000000000, 0x3faeea31c006c000, 0xbd0e113e4fc93b7b),
        (0x3fede00000000000, 0x3fb1973bd1466000, 0xbd25325d560d9e9b),
        (0x3feda00000000000, 0x3fb3bdf5a7d1e000, 0x3d2cc85ea5db4ed7),
        (0x3fed600000000000, 0x3fb5e95a4d97a000, 0xbd2c69063c5d1d1e),
        (0x3fed400000000000, 0x3fb700d30aeac000, 0x3cec1e8da99ded32),
        (0x3fed000000000000, 0x3fb9335e5d594000, 0x3d23115c3abd47da),
        (0x3fecc00000000000, 0x3fbb6ac88dad6000, 0xbd1390802bf768e5),
        (0x3feca00000000000, 0x3fbc885801bc4000, 0x3d2646d1c65aacd3),
        (0x3fec600000000000, 0x3fbec739830a2000, 0xbd2dc068afe645e0),
        (0x3fec400000000000, 0x3fbfe89139dbe000, 0xbd2534d64fa10afd),
        (0x3fec000000000000, 0x3fc1178e8227e000, 0x3d21ef78ce2d07f2),
        (0x3febe00000000000, 0x3fc1aa2b7e23f000, 0x3d2ca78e44389934),
        (0x3feba00000000000, 0x3fc2d1610c868000, 0x3d039d6ccb81b4a1),
        (0x3feb800000000000, 0x3fc365fcb0159000, 0x3cc62fa8234b7289),
        (0x3feb400000000000, 0x3fc4913d8333b000, 0x3d25837954fdb678),
        (0x3feb200000000000, 0x3fc527e5e4a1b000, 0x3d2633e8e5697dc7),
        (0x3feae00000000000, 0x3fc6574ebe8c1000, 0x3d19cf8b2c3c2e78),
        (0x3feac00000000000, 0x3fc6f0128b757000, 0xbd25118de59c21e1),
        (0x3feaa00000000000, 0x3fc7898d85445000, 0xbd1c661070914305),
        (0x3fea600000000000, 0x3fc8beafeb390000, 0xbd073d54aae92cd1),
        (0x3fea400000000000, 0x3fc95a5adcf70000, 0x3d07f22858a0ff6f),
        (0x3fea000000000000, 0x3fca93ed3c8ae000, 0xbd28724350562169),
        (0x3fe9e00000000000, 0x3fcb31d8575bd000, 0xbd0c358d4eace1aa),
        (0x3fe9c00000000000, 0x3fcbd087383be000, 0xbd2d4bc4595412b6),
        (0x3fe9a00000000000, 0x3fcc6ffbc6f01000, 0xbcf1ec72c5962bd2),
        (0x3fe9600000000000, 0x3fcdb13db0d49000, 0xbd2aff2af715b035),
        (0x3fe9400000000000, 0x3fce530effe71000, 0x3cc212276041f430),
        (0x3fe9200000000000, 0x3fcef5ade4dd0000, 0xbcca211565bb8e11),
        (0x3fe9000000000000, 0x3fcf991c6cb3b000, 0x3d1bcbecca0cdf30),
        (0x3fe8c00000000000, 0x3fd07138604d5800, 0x3cf89cdb16ed4e91),
        (0x3fe8a00000000000, 0x3fd0c42d67616000, 0x3d27188b163ceae9),
        (0x3fe8800000000000, 0x3fd1178e8227e800, 0xbd2c210e63a5f01c),
        (0x3fe8600000000000, 0x3fd16b5ccbacf800, 0x3d2b9acdf7a51681),
        (0x3fe8400000000000, 0x3fd1bf99635a6800, 0x3d2ca6ed5147bdb7),
        (0x3fe8200000000000, 0x3fd214456d0eb800, 0x3d0a87deba46baea),
        (0x3fe7e00000000000, 0x3fd2bef07cdc9000, 0x3d2a9cfa4a5004f4),
        (0x3fe7c00000000000, 0x3fd314f1e1d36000, 0xbd28e27ad3213cb8),
        (0x3fe7a00000000000, 0x3fd36b6776be1000, 0x3d116ecdb0f177c8),
        (0x3fe7800000000000, 0x3fd3c25277333000, 0x3d183b54b606bd5c),
        (0x3fe7600000000000, 0x3fd419b423d5e800, 0x3d08e436ec90e09d),
        (0x3fe7400000000000, 0x3fd4718dc271c800, 0xbd2f27ce0967d675),
        (0x3fe7200000000000, 0x3fd4c9e09e173000, 0xbd2e20891b0ad8a4),
        (0x3fe7000000000000, 0x3fd522ae0738a000, 0x3d2ebe708164c759),
        (0x3fe6e00000000000, 0x3fd57bf753c8d000, 0x3d1fadedee5d40ef),
        (0x3fe6c00000000000, 0x3fd5d5bddf596000, 0xbd0a0b2a08a465dc),
    ];
}

use pow_log_data::{LN2HI, LN2LO, POLY, TAB};

const N: u64 = 128;
const OFF: u64 = 0x3fe6955500000000;
const SIGN_BIAS: u64 = 0x800 << 7;

#[inline(always)]
fn d(b: u64) -> f64 {
    f64::from_bits(b)
}

/// The top 12 bits of the representation (sign and exponent).
#[inline(always)]
fn top12(x: f64) -> u32 {
    (x.to_bits() >> 52) as u32
}

/// `log_inline(ix)`: `log(x)` as `(y, tail)` in extended precision, `ix` normalized.
#[inline(always)]
fn log_inline(ix: u64) -> (f64, f64) {
    let tmp = ix.wrapping_sub(OFF);
    let i = ((tmp >> (52 - 7)) % N) as usize;
    let k = ((tmp as i64) >> 52) as i32;
    let iz = ix.wrapping_sub(tmp & (0xfffu64 << 52));
    let z = d(iz);
    let kd = f64::from(k);
    let (invc, logc, logctail) = (d(TAB[i].0), d(TAB[i].1), d(TAB[i].2));
    let a = |n: usize| d(POLY[n]);
    // log(x) = k ln2 + log(c) + log1p(z/c - 1); r = z/c - 1, exact with HAVE_FAST_FMA
    let r = z.mul_add(invc, -1.0);
    // k ln2 + log(c) + r
    let t1 = kd.mul_add(d(LN2HI), logc);
    let t2 = t1 + r;
    let lo1 = kd.mul_add(d(LN2LO), logctail);
    let lo2 = t1 - t2 + r;
    // log1p(r) = r - r^2/2 + r^3/3 + ...
    let ar = a(0) * r;
    let ar2 = r * ar;
    let ar3 = r * ar2;
    // k ln2 + log(c) + r + A[0] r^2
    let hi = t2 + ar2;
    let lo3 = ar.mul_add(r, -ar2);
    let lo4 = t2 - hi + ar2;
    // p = ar3 * (A1 + r*A2 + ar2*(A3 + r*A4 + ar2*(A5 + r*A6))), contracted as GCC did
    let a12 = r.mul_add(a(2), a(1));
    let a34 = r.mul_add(a(4), a(3));
    let a56 = r.mul_add(a(6), a(5));
    let inner = ar2.mul_add(a56, a34);
    let poly = ar2.mul_add(inner, a12);
    let lo = ar3.mul_add(poly, lo1 + lo2 + lo3 + lo4);
    let y = hi + lo;
    let tail = hi - y + lo;
    (y, tail)
}

/// `__math_oflow(sign)` / `__math_uflow(sign)`: `(sign ? -x : x) * x`.
#[inline(always)]
fn xflow(sign: bool, x: f64) -> f64 {
    (if sign { -x } else { x }) * x
}

#[inline(always)]
fn oflow(sign: bool) -> f64 {
    xflow(sign, d(0x7000000000000000)) // 0x1p769
}

#[inline(always)]
fn uflow(sign: bool) -> f64 {
    xflow(sign, d(0x1000000000000000)) // 0x1p-767
}

/// `specialcase(tmp, sbits, ki)` of `pow.c`: a result that may overflow or underflow, its sign
/// in `sbits`.
#[inline(always)]
fn specialcase(tmp: f64, sbits: u64, ki: u64) -> f64 {
    if ki & 0x80000000 == 0 {
        // k > 0: the exponent of scale might have overflowed by <= 460.
        let scale = d(sbits.wrapping_sub(1009u64 << 52));
        return d(0x7f00000000000000) * scale.mul_add(tmp, scale); // 0x1p1009
    }
    // k < 0: the exponent of scale might have underflowed by <= 1022 + 1009.
    let sbits = sbits.wrapping_add(1022u64 << 52);
    let scale = d(sbits);
    let st = scale * tmp;
    let mut y = scale + st;
    if y.abs() < 1.0 {
        // Round y to the right precision before scaling it into the subnormal range, to avoid
        // a double rounding; `one` has y's sign.
        let one = if y < 0.0 { -1.0 } else { 1.0 };
        let mut lo = scale - y + st;
        let hi = one + y;
        lo = one - hi + y + lo;
        y = (hi + lo) - one;
        // Fix the sign of 0.
        if y == 0.0 {
            y = d(sbits & 0x8000000000000000);
        }
    }
    d(0x0010000000000000) * y // 0x1p-1022
}

/// `exp_inline(x, xtail, sign_bias)`: `exp(x + xtail)`, negated when `sign_bias` is set.
#[inline(always)]
fn exp_inline(x: f64, xtail: f64, sign_bias: u64) -> f64 {
    use exp_data::{INVLN2N, NEGLN2HIN, NEGLN2LON, POLY as EPOLY, SHIFT, TAB as ETAB};
    let mut abstop = top12(x) & 0x7ff;
    if abstop.wrapping_sub(0x3c9) >= 0x408 - 0x3c9 {
        if abstop.wrapping_sub(0x3c9) >= 0x80000000 {
            // Avoid spurious underflow for tiny x; the result is ±(1 + x) rounded.
            let one = 1.0 + x;
            return if sign_bias != 0 { -one } else { one };
        }
        if abstop >= 0x409 {
            // |x| >= 1024: overflow or underflow.
            return if x.to_bits() >> 63 != 0 {
                uflow(sign_bias != 0)
            } else {
                oflow(sign_bias != 0)
            };
        }
        // Large x is special-cased below.
        abstop = 0;
    }
    let shift = d(SHIFT);
    let mut kd = x.mul_add(d(INVLN2N), shift); // z + Shift, z = InvLn2N * x, fused
    let ki = kd.to_bits();
    kd -= shift;
    let mut r = kd.mul_add(d(NEGLN2LON), kd.mul_add(d(NEGLN2HIN), x));
    // The code assumes 2^-200 < |xtail| < 2^-8/N.
    r = xtail + r;
    let idx = (2 * (ki % N)) as usize;
    let top = ki.wrapping_add(sign_bias) << (52 - 7);
    let tail = d(ETAB[idx]);
    let sbits = ETAB[idx + 1].wrapping_add(top);
    let r2 = r * r;
    let c = |n: usize| d(EPOLY[n]);
    let p23 = r.mul_add(c(1), c(0));
    let p45 = r.mul_add(c(3), c(2));
    let tmp = (r2 * r2).mul_add(p45, r2.mul_add(p23, tail + r));
    if abstop == 0 {
        return specialcase(tmp, sbits, ki);
    }
    let scale = d(sbits);
    scale.mul_add(tmp, scale)
}

/// `checkint(iy)`: 0 if `y` is not an integer, 1 if an odd one, 2 if an even one (`iy`: a
/// non-zero finite value's bits).
#[inline(always)]
fn checkint(iy: u64) -> i32 {
    let e = ((iy >> 52) & 0x7ff) as i32;
    if e < 0x3ff {
        return 0;
    }
    if e > 0x3ff + 52 {
        return 2;
    }
    if iy & ((1u64 << (0x3ff + 52 - e)) - 1) != 0 {
        return 0;
    }
    if iy & (1u64 << (0x3ff + 52 - e)) != 0 {
        return 1;
    }
    2
}

/// `zeroinfnan(i)`: the representation of 0, an infinity or a NaN.
#[inline(always)]
fn zeroinfnan(i: u64) -> bool {
    (2u64.wrapping_mul(i)).wrapping_sub(1)
        >= 2u64.wrapping_mul(f64::INFINITY.to_bits()).wrapping_sub(1)
}

/// `issignaling_inline(x)` with IEEE 754-2008 signalling NaNs (x86-64).
#[inline(always)]
fn issignaling(x: f64) -> bool {
    x.is_nan() && x.to_bits() & 0x0008000000000000 == 0
}

/// glibc 2.35's `pow(x, y)` on x86-64 (the FMA build), with its bits on every platform. The
/// `pow@GLIBC_2.2.5` wrapper old programs bind returns the same values.
#[inline(always)]
#[must_use]
pub fn pow_glibc(x: f64, y: f64) -> f64 {
    let mut sign_bias: u64 = 0;
    let mut ix = x.to_bits();
    let iy = y.to_bits();
    let mut topx = top12(x);
    let topy = top12(y);
    if topx.wrapping_sub(0x001) >= 0x7ff - 0x001
        || (topy & 0x7ff).wrapping_sub(0x3be) >= 0x43e - 0x3be
    {
        // x is subnormal, zero, negative, inf or nan, or |y| is tiny or huge (or inf or nan).
        if zeroinfnan(iy) {
            if 2u64.wrapping_mul(iy) == 0 {
                return if issignaling(x) { add_f64(x, y) } else { 1.0 };
            }
            if ix == 1f64.to_bits() {
                return if issignaling(y) { add_f64(x, y) } else { 1.0 };
            }
            let inf2 = 2u64.wrapping_mul(f64::INFINITY.to_bits());
            if 2u64.wrapping_mul(ix) > inf2 || 2u64.wrapping_mul(iy) > inf2 {
                return add_f64(x, y);
            }
            if 2u64.wrapping_mul(ix) == 2u64.wrapping_mul(1f64.to_bits()) {
                return 1.0;
            }
            if (2u64.wrapping_mul(ix) < 2u64.wrapping_mul(1f64.to_bits())) == (iy >> 63 == 0) {
                // |x| < 1 && y == inf or |x| > 1 && y == -inf
                return 0.0;
            }
            return y * y;
        }
        if zeroinfnan(ix) {
            let mut x2 = mul_f64(x, x);
            if ix >> 63 != 0 && checkint(iy) == 1 {
                x2 = -x2;
            }
            return if iy >> 63 != 0 { div_f64(1.0, x2) } else { x2 };
        }
        // Here x and y are non-zero finite.
        if ix >> 63 != 0 {
            // Finite x < 0.
            let yint = checkint(iy);
            if yint == 0 {
                return invalid_f64(x);
            }
            if yint == 1 {
                sign_bias = SIGN_BIAS;
            }
            ix &= 0x7fffffffffffffff;
            topx &= 0x7ff;
        }
        if (topy & 0x7ff).wrapping_sub(0x3be) >= 0x43e - 0x3be {
            // Note: sign_bias == 0 here because y is not odd.
            if ix == 1f64.to_bits() {
                return 1.0;
            }
            if (topy & 0x7ff) < 0x3be {
                // |y| < 2^-65, x^y ~= 1 + y*log(x).
                return if ix > 1f64.to_bits() {
                    1.0 + y
                } else {
                    1.0 - y
                };
            }
            return if (ix > 1f64.to_bits()) == (topy < 0x800) {
                oflow(false)
            } else {
                uflow(false)
            };
        }
        if topx == 0 {
            // Normalize a subnormal x so its exponent becomes negative.
            ix = (x * d(0x4330000000000000)).to_bits(); // 0x1p52
            ix &= 0x7fffffffffffffff;
            ix = ix.wrapping_sub(52u64 << 52);
        }
    }
    let (hi, lo) = log_inline(ix);
    let ehi = y * hi;
    let elo = y.mul_add(lo, y.mul_add(hi, -ehi));
    exp_inline(ehi, elo, sign_bias)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_and_special_values() {
        assert_eq!(pow_glibc(10.0, 2.0), 100.0);
        assert_eq!(pow_glibc(10.0, 11.0), 1e11);
        assert_eq!(pow_glibc(2.0, 0.5), std::f64::consts::SQRT_2);
        assert_eq!(pow_glibc(0.0, 0.275), 0.0);
        assert_eq!(pow_glibc(1.0, f64::NAN), 1.0);
        assert_eq!(pow_glibc(-2.0, 3.0), -8.0);
        assert_eq!(pow_glibc(f64::INFINITY, -1.0), 0.0);
        assert_eq!(pow_glibc(1e300, 2.0), f64::INFINITY);
        assert_eq!(pow_glibc(1e-300, 2.0), 0.0);
        assert_eq!(pow_glibc(5e-324, 0.5), 2.2227587494850775e-162);
        // SSE's default NaN for a negative number to a non-integer power, on every platform.
        assert_eq!(pow_glibc(-2.0, 0.5).to_bits(), 0xfff8_0000_0000_0000);
        // Two NaNs: x's, quieted (SSE's `x + y`).
        let (a, b) = (
            f64::from_bits(0x7ff0_0000_0000_0001),
            f64::from_bits(0xfff8_0000_0000_0002),
        );
        assert_eq!(pow_glibc(a, b).to_bits(), 0x7ff8_0000_0000_0001);
        assert_eq!(pow_glibc(b, a).to_bits(), 0xfff8_0000_0000_0002);
        for &(x, y) in &[
            (0.2f64, 0.275f64),
            (123456.7, 1.0 / 3.0),
            (8.0, 1.0 / 3.0),
            (0.5, 0.275),
        ] {
            let p = pow_glibc(x, y);
            let q = (y * x.ln()).exp();
            assert!((p - q).abs() <= 1e-15 * q, "{x} {y} {p} {q}");
        }
    }
}
