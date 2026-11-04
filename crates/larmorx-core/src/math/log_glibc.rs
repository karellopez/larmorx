// SPDX-License-Identifier: Apache-2.0 AND MIT
// Translated to Rust from Arm's Optimized Routines v19.11 (github.com/ARM-software/optimized-routines,
// tag v19.11): math/log.c, math/log_data.c (__log_data) and math/math_config.h. Copyright (c) 2018,
// Arm Limited, under the MIT licence (its text and notice are in
// LICENSES/MIT-arm-optimized-routines.txt and NOTICE). Rust translation Copyright 2026 Karel Lopez
// Vilaret; modified from the originals.
//! The natural logarithm for binary64 with the bits of glibc 2.35's `log` on x86-64 (the FMA
//! build, `__log_fma`, Arm's `log.c` compiled with `-mfma -mavx2`), on every platform.
//!
//! Every fused multiply-add of that machine code (read from `objdump -d` of Ubuntu 22.04's
//! libm.so.6) is a `mul_add` here: `fma(z, invc, -1)` the source asks for, and the ones GCC
//! contracted (`kd * Ln2hi + logc`, `kd * Ln2lo + lo`, the polynomials). Near 1 the split of `r`
//! into `rhi` and `rlo` is fused too, with the same bits (its products are exact). The NaN
//! results are SSE's (`super::x86_nan`). Not correctly rounded (Arm: 0.519 ulp; about 0.04 % of
//! calls); [`super::log`] is, and is larmorx's default. This one serves `reproduce` (FSL's FAST:
//! its log intensities; `fslmaths -log`). Until 2026-11-04 the FSL replicas' core carried its own
//! translation (`larmorx-nc-core`'s `glibc_double`); it now calls this one. Proved equal to the
//! system's `log` (glibc 2.35, x86-64) and to that translation on more than 10^9 arguments, NaN
//! bits included (`validation/investigations/glibc-maths/`).
//!
//! This is the current symbol (`log@@GLIBC_2.29`): the logarithm of a negative number is SSE's
//! default NaN (`-nan`). The older symbol an old C library's programs bind (`log@GLIBC_2.2.5`,
//! an SVID-compatible wrapper) gives C's positive `NAN` there; the FSL replicas keep that
//! wrapper (`larmorx-nc-core`'s `log_compat`).

#![allow(clippy::unreadable_literal)]

use super::x86_nan::invalid_f64;

/// `__log_data` (Arm's `log_data.c`, `LOG_TABLE_BITS` 7, `LOG_POLY_ORDER` 6, `LOG_POLY1_ORDER`
/// 12, with `HAVE_FAST_FMA`: no `tab2`), as 64-bit patterns (every word identical to Ubuntu
/// 22.04's libm.so.6, glibc 2.35-0ubuntu3: `validation/investigations/fsl-fast/arm_glibc_expf_log.py`).
#[rustfmt::skip]
mod log_data {
    /// `ln2hi`, `ln2lo`.
    pub(super) const LN2HI: u64 = 0x3fe62e42fefa3800;
    pub(super) const LN2LO: u64 = 0x3d2ef35793c76730;
    /// `poly` (A[0..5]).
    pub(super) const POLY: [u64; 5] = [0xbfe0000000000001, 0x3fd555555551305b, 0xbfcfffffffeb4590, 0x3fc999b324f10111, 0xbfc55575e506c89f];
    /// `poly1` (B[0..11]).
    pub(super) const POLY1: [u64; 11] = [
        0xbfe0000000000000,
        0x3fd5555555555577,
        0xbfcffffffffffdcb,
        0x3fc999999995dd0c,
        0xbfc55555556745a7,
        0x3fc24924a344de30,
        0xbfbfffffa4423d65,
        0x3fbc7184282ad6ca,
        0xbfb999eb43b068ff,
        0x3fb78182f7afd085,
        0xbfb5521375d145cd,
    ];
    /// `tab`: (invc, logc).
    pub(super) const TAB: [(u64, u64); 128] = [
        (0x3ff734f0c3e0de9f, 0xbfd7cc7f79e69000),
        (0x3ff713786a2ce91f, 0xbfd76feec20d0000),
        (0x3ff6f26008fab5a0, 0xbfd713e31351e000),
        (0x3ff6d1a61f138c7d, 0xbfd6b85b38287800),
        (0x3ff6b1490bc5b4d1, 0xbfd65d5590807800),
        (0x3ff69147332f0cba, 0xbfd602d076180000),
        (0x3ff6719f18224223, 0xbfd5a8ca86909000),
        (0x3ff6524f99a51ed9, 0xbfd54f4356035000),
        (0x3ff63356aa8f24c4, 0xbfd4f637c36b4000),
        (0x3ff614b36b9ddc14, 0xbfd49da7fda85000),
        (0x3ff5f66452c65c4c, 0xbfd445923989a800),
        (0x3ff5d867b5912c4f, 0xbfd3edf439b0b800),
        (0x3ff5babccb5b90de, 0xbfd396ce448f7000),
        (0x3ff59d61f2d91a78, 0xbfd3401e17bda000),
        (0x3ff5805612465687, 0xbfd2e9e2ef468000),
        (0x3ff56397cee76bd3, 0xbfd2941b3830e000),
        (0x3ff54725e2a77f93, 0xbfd23ec58cda8800),
        (0x3ff52aff42064583, 0xbfd1e9e129279000),
        (0x3ff50f22dbb2bddf, 0xbfd1956d2b48f800),
        (0x3ff4f38f4734ded7, 0xbfd141679ab9f800),
        (0x3ff4d843cfde2840, 0xbfd0edd094ef9800),
        (0x3ff4bd3ec078a3c8, 0xbfd09aa518db1000),
        (0x3ff4a27fc3e0258a, 0xbfd047e65263b800),
        (0x3ff4880524d48434, 0xbfcfeb224586f000),
        (0x3ff46dce1b192d0b, 0xbfcf474a7517b000),
        (0x3ff453d9d3391854, 0xbfcea4443d103000),
        (0x3ff43a2744b4845a, 0xbfce020d44e9b000),
        (0x3ff420b54115f8fb, 0xbfcd60a22977f000),
        (0x3ff40782da3ef4b1, 0xbfccc00104959000),
        (0x3ff3ee8f5d57fe8f, 0xbfcc202956891000),
        (0x3ff3d5d9a00b4ce9, 0xbfcb81178d811000),
        (0x3ff3bd60c010c12b, 0xbfcae2c9ccd3d000),
        (0x3ff3a5242b75dab8, 0xbfca45402e129000),
        (0x3ff38d22cd9fd002, 0xbfc9a877681df000),
        (0x3ff3755bc5847a1c, 0xbfc90c6d69483000),
        (0x3ff35dce49ad36e2, 0xbfc87120a645c000),
        (0x3ff34679984dd440, 0xbfc7d68fb4143000),
        (0x3ff32f5cceffcb24, 0xbfc73cb83c627000),
        (0x3ff3187775a10d49, 0xbfc6a39a9b376000),
        (0x3ff301c8373e3990, 0xbfc60b3154b7a000),
        (0x3ff2eb4ebb95f841, 0xbfc5737d76243000),
        (0x3ff2d50a0219a9d1, 0xbfc4dc7b8fc23000),
        (0x3ff2bef9a8b7fd2a, 0xbfc4462c51d20000),
        (0x3ff2a91c7a0c1bab, 0xbfc3b08abc830000),
        (0x3ff293726014b530, 0xbfc31b996b490000),
        (0x3ff27dfa5757a1f5, 0xbfc2875490a44000),
        (0x3ff268b39b1d3bbf, 0xbfc1f3b9f879a000),
        (0x3ff2539d838ff5bd, 0xbfc160c8252ca000),
        (0x3ff23eb7aac9083b, 0xbfc0ce7f57f72000),
        (0x3ff22a012ba940b6, 0xbfc03cdc49fea000),
        (0x3ff2157996cc4132, 0xbfbf57bdbc4b8000),
        (0x3ff201201dd2fc9b, 0xbfbe370896404000),
        (0x3ff1ecf4494d480b, 0xbfbd17983ef94000),
        (0x3ff1d8f5528f6569, 0xbfbbf9674ed8a000),
        (0x3ff1c52311577e7c, 0xbfbadc79202f6000),
        (0x3ff1b17c74cb26e9, 0xbfb9c0c3e7288000),
        (0x3ff19e010c2c1ab6, 0xbfb8a646b372c000),
        (0x3ff18ab07bb670bd, 0xbfb78d01b3ac0000),
        (0x3ff1778a25efbcb6, 0xbfb674f145380000),
        (0x3ff1648d354c31da, 0xbfb55e0e6d878000),
        (0x3ff151b990275fdd, 0xbfb4485cdea1e000),
        (0x3ff13f0ea432d24c, 0xbfb333d94d6aa000),
        (0x3ff12c8b7210f9da, 0xbfb22079f8c56000),
        (0x3ff11a3028ecb531, 0xbfb10e4698622000),
        (0x3ff107fbda8434af, 0xbfaffa6c6ad20000),
        (0x3ff0f5ee0f4e6bb3, 0xbfadda8d4a774000),
        (0x3ff0e4065d2a9fce, 0xbfabbcece4850000),
        (0x3ff0d244632ca521, 0xbfa9a1894012c000),
        (0x3ff0c0a77ce2981a, 0xbfa788583302c000),
        (0x3ff0af2f83c636d1, 0xbfa5715e67d68000),
        (0x3ff09ddb98a01339, 0xbfa35c8a49658000),
        (0x3ff08cabaf52e7df, 0xbfa149e364154000),
        (0x3ff07b9f2f4e28fb, 0xbf9e72c082eb8000),
        (0x3ff06ab58c358f19, 0xbf9a55f152528000),
        (0x3ff059eea5ecf92c, 0xbf963d62cf818000),
        (0x3ff04949cdd12c90, 0xbf9228fb8caa0000),
        (0x3ff038c6c6f0ada9, 0xbf8c317b20f90000),
        (0x3ff02865137932a9, 0xbf8419355daa0000),
        (0x3ff0182427ea7348, 0xbf781203c2ec0000),
        (0x3ff008040614b195, 0xbf60040979240000),
        (0x3fefe01ff726fa1a, 0x3f6feff384900000),
        (0x3fefa11cc261ea74, 0x3f87dc41353d0000),
        (0x3fef6310b081992e, 0x3f93cea3c4c28000),
        (0x3fef25f63ceeadcd, 0x3f9b9fc114890000),
        (0x3feee9c8039113e7, 0x3fa1b0d8ce110000),
        (0x3feeae8078cbb1ab, 0x3fa58a5bd001c000),
        (0x3fee741aa29d0c9b, 0x3fa95c8340d88000),
        (0x3fee3a91830a99b5, 0x3fad276aef578000),
        (0x3fee01e009609a56, 0x3fb07598e598c000),
        (0x3fedca01e577bb98, 0x3fb253f5e30d2000),
        (0x3fed92f20b7c9103, 0x3fb42edd8b380000),
        (0x3fed5cac66fb5cce, 0x3fb606598757c000),
        (0x3fed272caa5ede9d, 0x3fb7da76356a0000),
        (0x3fecf26e3e6b2ccd, 0x3fb9ab434e1c6000),
        (0x3fecbe6da2a77902, 0x3fbb78c7bb0d6000),
        (0x3fec8b266d37086d, 0x3fbd431332e72000),
        (0x3fec5894bd5d5804, 0x3fbf0a3171de6000),
        (0x3fec26b533bb9f8c, 0x3fc067152b914000),
        (0x3febf583eeece73f, 0x3fc147858292b000),
        (0x3febc4fd75db96c1, 0x3fc2266ecdca3000),
        (0x3feb951e0c864a28, 0x3fc303d7a6c55000),
        (0x3feb65e2c5ef3e2c, 0x3fc3dfc33c331000),
        (0x3feb374867c9888b, 0x3fc4ba366b7a8000),
        (0x3feb094b211d304a, 0x3fc5933928d1f000),
        (0x3feadbe885f2ef7e, 0x3fc66acd2418f000),
        (0x3feaaf1d31603da2, 0x3fc740f8ec669000),
        (0x3fea82e63fd358a7, 0x3fc815c0f51af000),
        (0x3fea5740ef09738b, 0x3fc8e92954f68000),
        (0x3fea2c2a90ab4b27, 0x3fc9bb3602f84000),
        (0x3fea01a01393f2d1, 0x3fca8bed1c2c0000),
        (0x3fe9d79f24db3c1b, 0x3fcb5b515c01d000),
        (0x3fe9ae2505c7b190, 0x3fcc2967ccbcc000),
        (0x3fe9852ef297ce2f, 0x3fccf635d5486000),
        (0x3fe95cbaeea44b75, 0x3fcdc1bd3446c000),
        (0x3fe934c69de74838, 0x3fce8c01b8cfe000),
        (0x3fe90d4f2f6752e6, 0x3fcf5509c0179000),
        (0x3fe8e6528effd79d, 0x3fd00e6c121fb800),
        (0x3fe8bfce9fcc007c, 0x3fd071b80e93d000),
        (0x3fe899c0dabec30e, 0x3fd0d46b9e867000),
        (0x3fe87427aa2317fb, 0x3fd13687334bd000),
        (0x3fe84f00acb39a08, 0x3fd1980d67234800),
        (0x3fe82a49e8653e55, 0x3fd1f8ffe0cc8000),
        (0x3fe8060195f40260, 0x3fd2595fd7636800),
        (0x3fe7e22563e0a329, 0x3fd2b9300914a800),
        (0x3fe7beb377dcb5ad, 0x3fd3187210436000),
        (0x3fe79baa679725c2, 0x3fd377266dec1800),
        (0x3fe77907f2170657, 0x3fd3d54ffbaf3000),
        (0x3fe756cadbd6130c, 0x3fd432eee32fe000),
    ];
}

#[inline(always)]
fn d(b: u64) -> f64 {
    f64::from_bits(b)
}

/// glibc 2.35's `log(x)` on x86-64 (the FMA build), with its bits on every platform: `+0` for 1,
/// `-∞` for `±0`, `+∞` for `+∞`, SSE's default NaN for a negative number or `-∞`, a NaN quieted.
#[inline(always)]
#[must_use]
pub fn log_glibc(x: f64) -> f64 {
    use log_data::*;
    const LO: u64 = 0x3fee_0000_0000_0000; // asuint64(1.0 - 0x1p-4)
    const HI: u64 = 0x3ff1_0900_0000_0000; // asuint64(1.0 + 0x1.09p-4)
    const LOG_OFF: u64 = 0x3fe6_0000_0000_0000;
    let mut ix = x.to_bits();
    let top = (ix >> 48) as u32;
    if ix.wrapping_sub(LO) < HI - LO {
        // close to 1.0
        if ix == 1.0f64.to_bits() {
            return 0.0;
        }
        let b = |n: usize| d(POLY1[n]);
        let r = x - 1.0;
        let r2 = r * r;
        let r3 = r * r2;
        // y = r3 * (B1 + r B2 + r2 B3 + r3 (B4 + r B5 + r2 B6 + r3 (B7 + r B8 + r2 B9 + r3 B10)))
        let s1 = r2.mul_add(b(3), r.mul_add(b(2), b(1)));
        let s4 = r2.mul_add(b(6), r.mul_add(b(5), b(4)));
        let s7 = r3.mul_add(b(10), r2.mul_add(b(9), r.mul_add(b(8), b(7))));
        let p = s7.mul_add(r3, s4).mul_add(r3, s1);
        // w = r * 0x1p27; rhi = r + w - w (fused: the same bits, r * 0x1p27 is exact)
        let k27 = d(0x41a0_0000_0000_0000);
        let rhi = (-k27).mul_add(r, k27.mul_add(r, r));
        let rlo = r - rhi;
        let rr = rhi * rhi;
        // w = rhi * rhi * B0; hi = r + w; lo = r - hi + w (fused)
        let hi = rr.mul_add(b(0), r);
        let lo = rr.mul_add(b(0), r - hi);
        // lo += B0 * rlo * (rhi + r); y = r3 * p + lo; y += hi
        let lo = (b(0) * rlo).mul_add(rhi + r, lo);
        return p.mul_add(r3, lo) + hi;
    }
    if top.wrapping_sub(0x0010) >= 0x7ff0 - 0x0010 {
        // x < 0x1p-1022 or inf or nan
        if ix.wrapping_mul(2) == 0 {
            // __math_divzero(1): -1/0
            return f64::NEG_INFINITY;
        }
        if ix == f64::INFINITY.to_bits() {
            return x;
        }
        if (top & 0x8000) != 0 || (top & 0x7ff0) == 0x7ff0 {
            return invalid_f64(x);
        }
        // a subnormal, normalized (`x * 0x1p52`, exact)
        ix = (x * d(0x4330_0000_0000_0000)).to_bits();
        ix = ix.wrapping_sub(52u64 << 52);
    }
    // x = 2^k z, z in [OFF, 2 OFF] and exact; c near the centre of z's subinterval.
    let tmp = ix.wrapping_sub(LOG_OFF);
    let i = ((tmp >> (52 - 7)) % 128) as usize;
    let k = ((tmp as i64) >> 52) as i32;
    let iz = ix.wrapping_sub(tmp & (0xfffu64 << 52));
    let (invc, logc) = (d(TAB[i].0), d(TAB[i].1));
    let z = d(iz);
    // log(x) = log1p(z/c - 1) + log(c) + k ln2; r = z/c - 1, exact with HAVE_FAST_FMA
    let r = z.mul_add(invc, -1.0);
    let kd = f64::from(k);
    // hi + lo = r + log(c) + k ln2
    let w = kd.mul_add(d(LN2HI), logc);
    let hi = w + r;
    let lo = kd.mul_add(d(LN2LO), w - hi + r);
    let a = |n: usize| d(POLY[n]);
    let r2 = r * r;
    // y = lo + r2 A0 + r r2 (A1 + r A2 + r2 (A3 + r A4)) + hi
    let p = r.mul_add(a(4), a(3)).mul_add(r2, r.mul_add(a(2), a(1)));
    let u = r2.mul_add(a(0), lo);
    (r * r2).mul_add(p, u) + hi
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values() {
        assert_eq!(log_glibc(1.0).to_bits(), 0);
        assert_eq!(log_glibc(0.0), f64::NEG_INFINITY);
        assert_eq!(log_glibc(-0.0), f64::NEG_INFINITY);
        assert_eq!(log_glibc(f64::INFINITY), f64::INFINITY);
        assert_eq!(log_glibc(2.0), std::f64::consts::LN_2);
        // SSE's default NaN, on every platform; a NaN operand quieted.
        assert_eq!(log_glibc(-1.0).to_bits(), 0xfff8_0000_0000_0000);
        assert_eq!(
            log_glibc(f64::NEG_INFINITY).to_bits(),
            0xfff8_0000_0000_0000
        );
        let snan = f64::from_bits(0x7ff0_0000_0000_0001);
        assert_eq!(log_glibc(snan).to_bits(), 0x7ff8_0000_0000_0001);
        for &x in &[
            5e-324f64, 1e-300, 0.25, 0.97, 1.0001, 1.05, 2.0, 1e10, 1e300,
        ] {
            let q = x.ln();
            assert!((log_glibc(x) - q).abs() <= 2.5e-16 * q.abs(), "{x}");
        }
    }
}
