// SPDX-License-Identifier: Apache-2.0 AND MIT
//! Correctly rounded power function `x^y` for binary32 (CORE-MATH's `cr_powf`), in pure Rust.
//!
//! The result is `x^y` rounded to the nearest `f32` (ties to even) for every input, so it is
//! bit-identical on every platform. glibc's `powf` (which ITK and ANTs call through
//! `std::pow(float, float)`) is not correctly rounded in about 0.05 % of calls; there the two
//! differ by one unit in the last place (`docs/findings/platform-math.md`).
//!
//! Port of CORE-MATH's `src/binary32/pow/powf.c` at commit 040ee482a8ca
//! (<https://core-math.gitlabpages.inria.fr/>), MIT licence, Copyright (c) 2022-2025 Alexei
//! Sibidanov and Paul Zimmermann (the copyright and permission notice are reproduced below).
//! The algorithm, tables and operation order are kept exactly; it is re-expressed in Rust for
//! round-to-nearest only. Differences from the C file:
//!
//! - `__builtin_fma(a, b, c)` is `a.mul_add(b, c)` (a single rounding on every target); Rust
//!   never contracts other expressions into an FMA.
//! - `roundeven_finite` is [`f64::round_ties_even`].
//! - Floating-point exception flags and `errno` are not modelled (the C code saves and
//!   restores the inexact flag around exact results); results are unchanged.
//! - Hex float literals are written as `f64` bit patterns, generated from the C source; each
//!   table line carries the original literal as a comment.

// Copyright (c) 2022-2025 Alexei Sibidanov and Paul Zimmermann.
//
// This file is part of the CORE-MATH project
// (https://core-math.gitlabpages.inria.fr/).
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

/// `muldd`: the double-double product `(xh + xl) · (ch + cl)`; returns `(hi, lo)`.
#[inline(always)]
fn muldd(xh: f64, xl: f64, ch: f64, cl: f64) -> (f64, f64) {
    let ahlh = ch * xl;
    let alhh = cl * xh;
    let ahhh = ch * xh;
    let mut ahhl = ch.mul_add(xh, -ahhh);
    ahhl += alhh + ahlh;
    let h = ahhh + ahhl;
    let l = (ahhh - h) + ahhl;
    (h, l)
}

/// `mulddd`: `(xh + xl) · c`; returns `(hi, lo)`.
#[inline(always)]
fn mulddd(xh: f64, xl: f64, c: f64) -> (f64, f64) {
    let ahlh = c * xl;
    let ahhh = c * xh;
    let mut ahhl = c.mul_add(xh, -ahhh);
    ahhl += ahlh;
    let h = ahhh + ahhl;
    let l = (ahhh - h) + ahhl;
    (h, l)
}

/// `polydd`: Horner evaluation of the double-double polynomial `c` at `xh + xl`.
fn polydd(xh: f64, xl: f64, c: &[[f64; 2]]) -> (f64, f64) {
    let mut i = c.len() - 1;
    let (mut ch, mut cl) = (c[i][0], c[i][1]);
    while i > 0 {
        i -= 1;
        (ch, cl) = muldd(xh, xl, ch, cl);
        let th = ch + c[i][0];
        let tl = (c[i][0] - th) + ch;
        ch = th;
        cl += tl + c[i][1];
    }
    (ch, cl)
}

fn isint(y0: f32) -> bool {
    let u = y0.to_bits();
    let ey = ((u >> 23) & 0xff) as i32 - 127;
    let s = ey + 9;
    if ey >= 0 {
        if s >= 32 {
            return true;
        }
        return u << s == 0;
    }
    u << 1 == 0
}

fn isodd(y0: f32) -> bool {
    let u = y0.to_bits();
    let ey = ((u >> 23) & 0xff) as i32 - 127;
    let s = ey + 9;
    let mut odd = false;
    if ey >= 0 {
        if s < 32 && u << s == 0 {
            odd = (u >> (32 - s)) & 1 != 0;
        }
        if s == 32 {
            odd = u & 1 != 0;
        }
    }
    odd
}

fn is_signaling(x: f32) -> bool {
    let u = x.to_bits() ^ 0x0040_0000;
    (u & 0x7fff_ffff) > 0x7fc0_0000
}

/// `is_exact`: whether `x^y` is exactly representable as an `f32`.
fn is_exact(x: f32, y: f32) -> bool {
    let v = x.to_bits();
    let w = y.to_bits();
    if (v << 1) != 0x7f00_0000 && (w << (32 - 16)) != 0 {
        return false;
    }
    if (v << 1) == 0x7f00_0000 {
        return true;
    }
    const XMAX: [u32; 16] = [
        0, 0xffffff, 4095, 255, 63, 27, 15, 9, 7, 5, 5, 3, 3, 3, 3, 3,
    ];
    if y >= 0.0 && isint(y) {
        let mut m = v & 0x7fffff;
        let mut e = ((v << 1) >> 24) as i32 - 0x96;
        if e >= -149 {
            m |= 0x800000;
        } else {
            e += 1;
        }
        let t = m.trailing_zeros();
        m >>= t;
        e += t as i32;
        if y == 0.0 || y == 1.0 {
            return true;
        }
        if m == 1 {
            let ye = y * e as f32;
            return (-149.0..128.0).contains(&ye);
        }
        if !(0.0..=15.0).contains(&y) {
            return false;
        }
        let y_int = y as i32;
        if m > XMAX[y_int as usize] {
            return false;
        }
        let t = 32 - m.leading_zeros() as i32;
        let ez = e * y_int + t;
        if ez <= -149 || 128 < ez {
            return false;
        }
        return e * y_int >= -149;
    }
    let mut n = w & 0x7fffff;
    let mut f = ((w << 1) >> 24) as i32 - 0x96;
    if f >= -149 {
        n |= 0x800000;
    } else {
        f += 1;
    }
    let t = n.trailing_zeros();
    n >>= t;
    f += t as i32;
    let mut m = v & 0x7fffff;
    let mut e = ((v << 1) >> 24) as i32 - 0x96;
    if e >= -149 {
        m |= 0x800000;
    } else {
        e += 1;
    }
    let t = m.trailing_zeros();
    m >>= t;
    e += t as i32;
    if y < 0.0 {
        if m != 1 {
            return false;
        }
        let ez = if f >= 0 {
            (if e >= 0 {
                -e.wrapping_shl(f as u32)
            } else {
                (-e).wrapping_shl(f as u32)
            })
            .wrapping_mul(n as i32)
        } else {
            let t = e.trailing_zeros() as i32;
            if -f > t {
                return false;
            }
            ((-e) >> (-f)).wrapping_mul(n as i32)
        };
        return (-149..128).contains(&ez);
    }
    while f != 0 {
        f += 1;
        if e & 1 != 0 {
            return false;
        }
        e /= 2;
        let dm = m as f32;
        let s = dm.sqrt().round();
        if s * s != dm {
            return false;
        }
        m = s as u32;
    }
    if m > 1 {
        if 15 < n {
            return false;
        }
        if m > XMAX[n as usize] {
            return false;
        }
    }
    let mut my = m;
    let mut n0 = n;
    while n0 > 1 {
        my = my.wrapping_mul(m);
        n0 -= 1;
    }
    let t = 32 - my.leading_zeros() as i32;
    -149 <= e * n as i32 && e * n as i32 + t <= 128
}
/// `ix`: about `1 / (1 + j/32)`, the reduction factors of the fast path.
const IX: [f64; 33] = [
    f64::from_bits(0x3ff0000000000000), // 0x1p+0
    f64::from_bits(0x3fef07c1f07c0000), // 0x1.f07c1f07cp-1
    f64::from_bits(0x3fee1e1e1e1e0000), // 0x1.e1e1e1e1ep-1
    f64::from_bits(0x3fed41d41d420000), // 0x1.d41d41d42p-1
    f64::from_bits(0x3fec71c71c720000), // 0x1.c71c71c72p-1
    f64::from_bits(0x3febacf914c20000), // 0x1.bacf914c2p-1
    f64::from_bits(0x3feaf286bca20000), // 0x1.af286bca2p-1
    f64::from_bits(0x3fea41a41a420000), // 0x1.a41a41a42p-1
    f64::from_bits(0x3fe99999999a0000), // 0x1.99999999ap-1
    f64::from_bits(0x3fe8f9c18f9c0000), // 0x1.8f9c18f9cp-1
    f64::from_bits(0x3fe8618618620000), // 0x1.861861862p-1
    f64::from_bits(0x3fe7d05f417d0000), // 0x1.7d05f417dp-1
    f64::from_bits(0x3fe745d1745d0000), // 0x1.745d1745dp-1
    f64::from_bits(0x3fe6c16c16c10000), // 0x1.6c16c16c1p-1
    f64::from_bits(0x3fe642c8590b0000), // 0x1.642c8590bp-1
    f64::from_bits(0x3fe5c9882b930000), // 0x1.5c9882b93p-1
    f64::from_bits(0x3fe5555555550000), // 0x1.555555555p-1
    f64::from_bits(0x3fe4e5e0a72f0000), // 0x1.4e5e0a72fp-1
    f64::from_bits(0x3fe47ae147ae0000), // 0x1.47ae147aep-1
    f64::from_bits(0x3fe4141414140000), // 0x1.414141414p-1
    f64::from_bits(0x3fe3b13b13b10000), // 0x1.3b13b13b1p-1
    f64::from_bits(0x3fe3521cfb2b0000), // 0x1.3521cfb2bp-1
    f64::from_bits(0x3fe2f684bda10000), // 0x1.2f684bda1p-1
    f64::from_bits(0x3fe29e4129e40000), // 0x1.29e4129e4p-1
    f64::from_bits(0x3fe2492492490000), // 0x1.249249249p-1
    f64::from_bits(0x3fe1f7047dc10000), // 0x1.1f7047dc1p-1
    f64::from_bits(0x3fe1a7b9611a0000), // 0x1.1a7b9611ap-1
    f64::from_bits(0x3fe15b1e5f750000), // 0x1.15b1e5f75p-1
    f64::from_bits(0x3fe1111111110000), // 0x1.111111111p-1
    f64::from_bits(0x3fe0c9714fbd0000), // 0x1.0c9714fbdp-1
    f64::from_bits(0x3fe0842108420000), // 0x1.084210842p-1
    f64::from_bits(0x3fe0410410410000), // 0x1.041041041p-1
    f64::from_bits(0x3fe0000000000000), // 0x1p-1
];
/// `lix`: `log2(ix[j])` as a double-double (high part with few bits).
const LIX: [[f64; 2]; 33] = [
    [
        f64::from_bits(0x0000000000000000),
        f64::from_bits(0x0000000000000000),
    ], // 0x0p+0, 0x0p+0
    [
        f64::from_bits(0xbfa6c00000000000),
        f64::from_bits(0x3f04b229b87f3f89),
    ], // -0x1.6cp-5, 0x1.4b229b87f3f89p-15
    [
        f64::from_bits(0xbfb6600000000000),
        f64::from_bits(0xbf0fb7d654235799),
    ], // -0x1.66p-4, -0x1.fb7d654235799p-15
    [
        f64::from_bits(0xbfc0800000000000),
        f64::from_bits(0xbf38b119b2c9c87b),
    ], // -0x1.08p-3, -0x1.8b119b2c9c87bp-12
    [
        f64::from_bits(0xbfc5c00000000000),
        f64::from_bits(0xbeca39fa6533294d),
    ], // -0x1.5cp-3, -0x1.a39fa6533294dp-19
    [
        f64::from_bits(0xbfcac00000000000),
        f64::from_bits(0xbf3ebc5b663dd4b8),
    ], // -0x1.acp-3, -0x1.ebc5b663dd4b8p-12
    [
        f64::from_bits(0xbfcfc00000000000),
        f64::from_bits(0x3f1f4a37fe0fa46f),
    ], // -0x1.fcp-3, 0x1.f4a37fe0fa46fp-14
    [
        f64::from_bits(0xbfd2400000000000),
        f64::from_bits(0xbf301eac33103e6b),
    ], // -0x1.24p-2, -0x1.01eac33103e6bp-12
    [
        f64::from_bits(0xbfd4a00000000000),
        f64::from_bits(0x3f361ed0d15725e0),
    ], // -0x1.4ap-2, 0x1.61ed0d15725ep-12
    [
        f64::from_bits(0xbfd6e00000000000),
        f64::from_bits(0xbf210e6ceb499ba9),
    ], // -0x1.6ep-2, -0x1.10e6ceb499ba9p-13
    [
        f64::from_bits(0xbfd9200000000000),
        f64::from_bits(0x3f3115db8ada837d),
    ], // -0x1.92p-2, 0x1.115db8ada837dp-12
    [
        f64::from_bits(0xbfdb400000000000),
        f64::from_bits(0xbf3fafdce266d7ae),
    ], // -0x1.b4p-2, -0x1.fafdce266d7aep-12
    [
        f64::from_bits(0xbfdd600000000000),
        f64::from_bits(0xbf3d4f80cd19906f),
    ], // -0x1.d6p-2, -0x1.d4f80cd19906fp-12
    [
        f64::from_bits(0xbfdf800000000000),
        f64::from_bits(0x3f35ea5ccd0a7396),
    ], // -0x1.f8p-2, 0x1.5ea5ccd0a7396p-12
    [
        f64::from_bits(0x3fde800000000000),
        f64::from_bits(0xbf20500d67fe62eb),
    ], // 0x1.e8p-2, -0x1.0500d67fe62ebp-13
    [
        f64::from_bits(0x3fdc800000000000),
        f64::from_bits(0x3f19dc2d41aa4626),
    ], // 0x1.c8p-2, 0x1.9dc2d41aa4626p-14
    [
        f64::from_bits(0x3fda800000000000),
        f64::from_bits(0x3f4ff2e2ff321344),
    ], // 0x1.a8p-2, 0x1.ff2e2ff321344p-11
    [
        f64::from_bits(0x3fd8a00000000000),
        f64::from_bits(0x3f4130157f4c3a3e),
    ], // 0x1.8ap-2, 0x1.130157f4c3a3ep-11
    [
        f64::from_bits(0x3fd6c00000000000),
        f64::from_bits(0x3f461ed0cad929cc),
    ], // 0x1.6cp-2, 0x1.61ed0cad929ccp-11
    [
        f64::from_bits(0x3fd5000000000000),
        f64::from_bits(0xbf42089a632d7949),
    ], // 0x1.5p-2, -0x1.2089a632d7949p-11
    [
        f64::from_bits(0x3fd3200000000000),
        f64::from_bits(0x3f47fdc6dfb2d21a),
    ], // 0x1.32p-2, 0x1.7fdc6dfb2d21ap-11
    [
        f64::from_bits(0x3fd1600000000000),
        f64::from_bits(0x3f4380a6c36088f3),
    ], // 0x1.16p-2, 0x1.380a6c36088f3p-11
    [
        f64::from_bits(0x3fcf600000000000),
        f64::from_bits(0xbed3ab7dc7ba81ac),
    ], // 0x1.f6p-3, -0x1.3ab7dc7ba81acp-18
    [
        f64::from_bits(0x3fcc000000000000),
        f64::from_bits(0xbf1cc2c0061ef1a2),
    ], // 0x1.cp-3, -0x1.cc2c0061ef1a2p-14
    [
        f64::from_bits(0x3fc8a00000000000),
        f64::from_bits(0x3f3130157c97bbe0),
    ], // 0x1.8ap-3, 0x1.130157c97bbep-12
    [
        f64::from_bits(0x3fc5600000000000),
        f64::from_bits(0x3f1ee14ff34c4128),
    ], // 0x1.56p-3, 0x1.ee14ff34c4128p-14
    [
        f64::from_bits(0x3fc2200000000000),
        f64::from_bits(0x3f3b5b854c4fde69),
    ], // 0x1.22p-3, 0x1.b5b854c4fde69p-12
    [
        f64::from_bits(0x3fbe000000000000),
        f64::from_bits(0x3f2635d1df7cb0b5),
    ], // 0x1.ep-4, 0x1.635d1df7cb0b5p-13
    [
        f64::from_bits(0x3fb7e00000000000),
        f64::from_bits(0xbf23f6d2636c101e),
    ], // 0x1.7ep-4, -0x1.3f6d2636c101ep-13
    [
        f64::from_bits(0x3fb1c00000000000),
        f64::from_bits(0xbf133567f1b193a4),
    ], // 0x1.1cp-4, -0x1.33567f1b193a4p-14
    [
        f64::from_bits(0x3fa7800000000000),
        f64::from_bits(0xbf18d66c5313a71d),
    ], // 0x1.78p-5, -0x1.8d66c5313a71dp-14
    [
        f64::from_bits(0x3f97400000000000),
        f64::from_bits(0x3eef7430ee200e00),
    ], // 0x1.74p-6, 0x1.f7430ee200ep-17
    [
        f64::from_bits(0x0000000000000000),
        f64::from_bits(0x0000000000000000),
    ], // 0x0p+0, 0x0p+0
];
/// `c`: the fast path's log2 polynomial.
const C: [f64; 8] = [
    f64::from_bits(0x3ff71547652b82fe), // 0x1.71547652b82fep+0
    f64::from_bits(0xbfe71547652b82fe), // -0x1.71547652b82fep-1
    f64::from_bits(0x3fdec709dc3a2d0b), // 0x1.ec709dc3a2d0bp-2
    f64::from_bits(0xbfd71547652bc4a9), // -0x1.71547652bc4a9p-2
    f64::from_bits(0x3fd2776c441b72e0), // 0x1.2776c441b72ep-2
    f64::from_bits(0xbfcec709bdf453ec), // -0x1.ec709bdf453ecp-3
    f64::from_bits(0x3fca6406efd4b877), // 0x1.a6406efd4b877p-3
    f64::from_bits(0xbfc717d824a520f7), // -0x1.717d824a520f7p-3
];
/// `ce` of `cr_powf`: the fast path's exp2 polynomial.
const CE_FAST: [f64; 6] = [
    f64::from_bits(0x3fa62e42fefa398b), // 0x1.62e42fefa398bp-5
    f64::from_bits(0x3f4ebfbdff84555a), // 0x1.ebfbdff84555ap-11
    f64::from_bits(0x3eec6b08d4ad86d3), // 0x1.c6b08d4ad86d3p-17
    f64::from_bits(0x3e83b2ad1b1716a2), // 0x1.3b2ad1b1716a2p-23
    f64::from_bits(0x3e15d7472718ce9d), // 0x1.5d7472718ce9dp-30
    f64::from_bits(0x3da4a1d7f457ac56), // 0x1.4a1d7f457ac56p-37
];
/// `tb`: `2^(i/16)`.
const TB: [f64; 16] = [
    f64::from_bits(0x3ff0000000000000), // 0x1p+0
    f64::from_bits(0x3ff0b5586cf9890f), // 0x1.0b5586cf9890fp+0
    f64::from_bits(0x3ff172b83c7d517b), // 0x1.172b83c7d517bp+0
    f64::from_bits(0x3ff2387a6e756238), // 0x1.2387a6e756238p+0
    f64::from_bits(0x3ff306fe0a31b715), // 0x1.306fe0a31b715p+0
    f64::from_bits(0x3ff3dea64c123422), // 0x1.3dea64c123422p+0
    f64::from_bits(0x3ff4bfdad5362a27), // 0x1.4bfdad5362a27p+0
    f64::from_bits(0x3ff5ab07dd485429), // 0x1.5ab07dd485429p+0
    f64::from_bits(0x3ff6a09e667f3bcd), // 0x1.6a09e667f3bcdp+0
    f64::from_bits(0x3ff7a11473eb0187), // 0x1.7a11473eb0187p+0
    f64::from_bits(0x3ff8ace5422aa0db), // 0x1.8ace5422aa0dbp+0
    f64::from_bits(0x3ff9c49182a3f090), // 0x1.9c49182a3f09p+0
    f64::from_bits(0x3ffae89f995ad3ad), // 0x1.ae89f995ad3adp+0
    f64::from_bits(0x3ffc199bdd85529c), // 0x1.c199bdd85529cp+0
    f64::from_bits(0x3ffd5818dcfba487), // 0x1.d5818dcfba487p+0
    f64::from_bits(0x3ffea4afa2a490da), // 0x1.ea4afa2a490dap+0
];
/// `ch` of `as_powf_accurate2`: the accurate log2 polynomial (double-double).
const CH: [[f64; 2]; 13] = [
    [
        f64::from_bits(0x40071547652b82fe),
        f64::from_bits(0x3c8777d0ffda2b89),
    ], // 0x1.71547652b82fep+1, 0x1.777d0ffda2b89p-55
    [
        f64::from_bits(0x3feec709dc3a03fd),
        f64::from_bits(0x3c8d27f04ff73b3a),
    ], // 0x1.ec709dc3a03fdp-1, 0x1.d27f04ff73b3ap-55
    [
        f64::from_bits(0x3fe2776c50ef9bfe),
        f64::from_bits(0x3c8e4b514251d0ec),
    ], // 0x1.2776c50ef9bfep-1, 0x1.e4b514251d0ecp-55
    [
        f64::from_bits(0x3fda61762a7aded9),
        f64::from_bits(0x3c6de632dc7f6998),
    ], // 0x1.a61762a7aded9p-2, 0x1.de632dc7f6998p-57
    [
        f64::from_bits(0x3fd484b13d7c02ae),
        f64::from_bits(0x3c7a320ec342ddb3),
    ], // 0x1.484b13d7c02aep-2, 0x1.a320ec342ddb3p-56
    [
        f64::from_bits(0x3fd0c9a84993fd48),
        f64::from_bits(0xbc6e6425ce9a74a4),
    ], // 0x1.0c9a84993fd48p-2, -0x1.e6425ce9a74a4p-57
    [
        f64::from_bits(0x3fcc68f568d8beaf),
        f64::from_bits(0xbc603a175487feab),
    ], // 0x1.c68f568d8beafp-3, -0x1.03a175487feabp-57
    [
        f64::from_bits(0x3fc89f3b14657dfb),
        f64::from_bits(0x3c6f04a3acf0bcf7),
    ], // 0x1.89f3b14657dfbp-3, 0x1.f04a3acf0bcf7p-57
    [
        f64::from_bits(0x3fc5b9ad2f2d12a0),
        f64::from_bits(0xbc568fdff6815a6f),
    ], // 0x1.5b9ad2f2d12ap-3, -0x1.68fdff6815a6fp-58
    [
        f64::from_bits(0x3fc3702165b88acb),
        f64::from_bits(0x3c345b052ace6c8e),
    ], // 0x1.3702165b88acbp-3, 0x1.45b052ace6c8ep-60
    [
        f64::from_bits(0x3fc1998f60f2f005),
        f64::from_bits(0xbc679a94f62fb524),
    ], // 0x1.1998f60f2f005p-3, -0x1.79a94f62fb524p-57
    [
        f64::from_bits(0x3fbf9bc428e30809),
        f64::from_bits(0xbc451f063387e470),
    ], // 0x1.f9bc428e30809p-4, -0x1.51f063387e47p-59
    [
        f64::from_bits(0x3fc1ac0ab871296a),
        f64::from_bits(0x3c62ba6a2e1a625b),
    ], // 0x1.1ac0ab871296ap-3, 0x1.2ba6a2e1a625bp-57
];
/// `ce` of `as_powf_accurate2`: the accurate exp2 polynomial (double-double).
const CE_ACC: [[f64; 2]; 18] = [
    [
        f64::from_bits(0x3ff0000000000000),
        f64::from_bits(0x39df7d70599926c4),
    ], // 0x1p+0, 0x1.f7d70599926c4p-98
    [
        f64::from_bits(0x3fe62e42fefa39ef),
        f64::from_bits(0x3c7abc9e3b39856b),
    ], // 0x1.62e42fefa39efp-1, 0x1.abc9e3b39856bp-56
    [
        f64::from_bits(0x3fcebfbdff82c58f),
        f64::from_bits(0xbc65e43a540c283d),
    ], // 0x1.ebfbdff82c58fp-3, -0x1.5e43a540c283dp-57
    [
        f64::from_bits(0x3fac6b08d704a0c0),
        f64::from_bits(0xbc4d3316277451e6),
    ], // 0x1.c6b08d704a0cp-5, -0x1.d3316277451e6p-59
    [
        f64::from_bits(0x3f83b2ab6fba4e77),
        f64::from_bits(0x3c14e66003ba7f85),
    ], // 0x1.3b2ab6fba4e77p-7, 0x1.4e66003ba7f85p-62
    [
        f64::from_bits(0x3f55d87fe78a6731),
        f64::from_bits(0x3bd07183d46a9697),
    ], // 0x1.5d87fe78a6731p-10, 0x1.07183d46a9697p-66
    [
        f64::from_bits(0x3f2430912f86c787),
        f64::from_bits(0x3bcbc81afca4c930),
    ], // 0x1.430912f86c787p-13, 0x1.bc81afca4c93p-67
    [
        f64::from_bits(0x3eeffcbfc588b0c7),
        f64::from_bits(0xbb8e63f6f0116f4c),
    ], // 0x1.ffcbfc588b0c7p-17, -0x1.e63f6f0116f4cp-71
    [
        f64::from_bits(0x3eb62c0223a5c826),
        f64::from_bits(0xbb530542d98ea4a5),
    ], // 0x1.62c0223a5c826p-20, -0x1.30542d98ea4a5p-74
    [
        f64::from_bits(0x3e7b5253d395e7c6),
        f64::from_bits(0xbaf9285a132ce05e),
    ], // 0x1.b5253d395e7c6p-24, -0x1.9285a132ce05ep-80
    [
        f64::from_bits(0x3e3e4cf5158b7b01),
        f64::from_bits(0xbac9ac1facae1b88),
    ], // 0x1.e4cf5158b7b01p-28, -0x1.9ac1facae1b88p-83
    [
        f64::from_bits(0x3dfe8cac7351a7a8),
        f64::from_bits(0xba44fb82adebd76b),
    ], // 0x1.e8cac7351a7a8p-32, -0x1.4fb82adebd76bp-91
    [
        f64::from_bits(0x3dbc3bd65182746d),
        f64::from_bits(0x3a484ad0689d30e0),
    ], // 0x1.c3bd65182746dp-36, 0x1.84ad0689d30ep-91
    [
        f64::from_bits(0x3d78161931d765c3),
        f64::from_bits(0xba0254c6535279ce),
    ], // 0x1.8161931d765c3p-40, -0x1.254c6535279cep-95
    [
        f64::from_bits(0x3d3314943a26c9e2),
        f64::from_bits(0xb9df4f2fdc14fb82),
    ], // 0x1.314943a26c9e2p-44, -0x1.f4f2fdc14fb82p-98
    [
        f64::from_bits(0x3cec36e53b459602),
        f64::from_bits(0x398f0d06a5a63c41),
    ], // 0x1.c36e53b459602p-49, 0x1.f0d06a5a63c41p-103
    [
        f64::from_bits(0x3ca397637b3876a4),
        f64::from_bits(0xb945632c551ae458),
    ], // 0x1.397637b3876a4p-53, -0x1.5632c551ae458p-107
    [
        f64::from_bits(0x3c598fbfefdddb51),
        f64::from_bits(0xb8cfd134923d52b4),
    ], // 0x1.98fbfefdddb51p-58, -0x1.fd134923d52b4p-115
];

/// The NaN of an invalid operation (`(x - x) / (x - x)` in the C code): x86-64's default NaN,
/// negative, as glibc's `powf` returns it there (the C would give the platform's own).
const INVALID: f32 = f32::from_bits(0xffc0_0000);

/// `x^y`, correctly rounded (CORE-MATH's `cr_powf`).
#[must_use]
pub fn powf(x0: f32, y0: f32) -> f32 {
    let x = f64::from(x0);
    let mut y = f64::from(y0);
    let tx = x.to_bits();
    let ty = y.to_bits();
    let mut xsgn = false;
    if tx << 1 == 0x3ffu64 << 53 {
        // |x| = 1
        if tx >> 63 != 0 {
            if (ty << 1) > 0x7ffu64 << 53 {
                return y0 + y0;
            }
            if isint(y0) {
                return if isodd(y0) { x0 } else { -x0 };
            }
            return INVALID;
        }
        return if is_signaling(y0) { x0 + y0 } else { x0 };
    }
    if ty << 1 == 0 {
        return if is_signaling(x0) { x0 + y0 } else { 1.0 };
    }
    if ty == 0x3ffu64 << 52 {
        return if is_signaling(x0) { x0 + y0 } else { x0 };
    }
    if (ty << 1) >= 0x7ffu64 << 53 {
        // y = ±inf or NaN
        if (tx << 1) > 0x7ffu64 << 53 {
            return x0 + y0;
        }
        if (ty << 1) == 0x7ffu64 << 53 {
            return if ((tx << 1) < (0x3ffu64 << 53)) ^ (ty >> 63 != 0) {
                0.0
            } else {
                f32::INFINITY
            };
        }
        return x0 + y0;
    }
    let mut x0 = x0;
    if tx >= 0x7ffu64 << 52 {
        // x is ±inf, NaN or negative
        if (tx << 1) == 0x7ffu64 << 53 {
            if !isodd(y0) {
                x0 = x0.abs();
            }
            return if ty >> 63 != 0 { 1.0 / x0 } else { x0 };
        }
        if (tx << 1) > 0x7ffu64 << 53 {
            return x0 + x0;
        }
        if tx > 0x7ffu64 << 52 {
            xsgn = true;
            if !isint(y0) && x != 0.0 {
                return INVALID;
            }
        }
    }
    if tx << 1 == 0 {
        // x = ±0
        return if ty >> 63 != 0 {
            if isodd(y0) {
                1.0 / 0.0f32.copysign(x0)
            } else {
                f32::INFINITY
            }
        } else if isodd(y0) {
            1.0f32.copysign(x0) * 0.0
        } else {
            0.0
        };
    }
    let m = tx & (!0u64 >> 12);
    let mut e = ((tx >> 52) & 0x7ff) as i32 - 0x3ff;
    let j = ((m + (1u64 << (52 - 6))) >> (52 - 5)) as usize;
    let k = i32::from(j > 13);
    e += k;
    let xd = f64::from_bits(m | 0x3ffu64 << 52);
    let mut z = xd.mul_add(IX[j], -1.0);
    let z2 = z * z;
    let z4 = z2 * z2;
    let c6 = C[6] + z * C[7];
    let c4 = C[4] + z * C[5];
    let c2 = C[2] + z * C[3];
    let mut c0 = C[0] + z * C[1];
    c0 += z2 * c2;
    let c4 = c4 + z2 * c6;
    c0 += z4 * c4;
    let l = z * c0 - LIX[j][1];
    y *= 16.0;
    let zt = (f64::from(e) - LIX[j][0]) * y;
    z = l * y + zt;
    if z > 2048.0 {
        return if isodd(y0) {
            f32::from_bits(0x7f00_0000).copysign(x0) * f32::from_bits(0x7f00_0000)
        } else {
            f32::INFINITY
        };
    }
    if z < -2400.0 {
        return if isodd(y0) {
            f32::from_bits(0x0080_0000).copysign(x0) * f32::from_bits(0x0080_0000)
        } else {
            0.0
        };
    }
    if z.abs() < f64::from_bits(0x3e50_0000_0000_0000) {
        // |z| < 2^-26
        return (1.0 + z) as f32;
    }
    let ia = z.floor();
    let h = l.mul_add(y, zt - ia);
    let il = ia as i64;
    let jl = il & 0xf;
    let el = (il - jl) >> 4;
    let mut s = TB[jl as usize];
    let su = f64::from_bits((el as u64).wrapping_add(0x3ff) << 52);
    s *= su;
    let h2 = h * h;
    let mut c0 = CE_FAST[0] + h * CE_FAST[1];
    let c2 = CE_FAST[2] + h * CE_FAST[3];
    let c4 = CE_FAST[4] + h * CE_FAST[5];
    c0 += h2 * (c2 + h2 * c4);
    let w = s * h;
    let mut rr = s + w * c0;
    let off = 468u64;
    if (rr.to_bits().wrapping_add(off) & 0xfff_ffff) <= 2 * off {
        return powf_accurate(x0, y0, is_exact(x0, y0));
    }
    if xsgn && isodd(y0) {
        rr = -rr;
    }
    rr as f32
}

/// `as_powf_accurate2`: the double-double path for results close to a rounding boundary.
fn powf_accurate(x0: f32, y0: f32, exact: bool) -> f32 {
    const O: [f64; 2] = [1.0, 2.0];
    let y = f64::from(y0);
    let mut t = f64::from(x0).to_bits();
    let mut e = ((t >> 52) & 0x7ff) as i32 - 0x3ff;
    let xsgn = t >> 63 != 0;
    t &= !0u64 >> 12;
    let k = usize::from(t > 0x6a09e667f3bcd);
    e += k as i32;
    t |= 0x3ffu64 << 52;
    let x = f64::from_bits(t);
    let xm = x - O[k];
    let xp = x + O[k];
    let zh = xm / xp;
    let zl = zh.mul_add(-xp, xm) / xp;
    let (z2h, z2l) = muldd(zh, zl, zh, zl);
    let (z2h, z2l) = polydd(z2h, z2l, &CH);
    let (zh, zl) = muldd(zh, zl, z2h, z2l);
    let (zh, zl) = mulddd(zh, zl, y);
    let ey = f64::from(e) * y;
    let mut eh = ey + zh;
    let el = ((ey - eh) + zh) + zl;
    let ee = eh.round_ties_even();
    eh -= ee;
    let (eh, el) = polydd(eh, el, &CE_ACC);
    let r = f64::from_bits(((0x3ff_i64 + ee as i64) as u64) << 52);
    let mut lh = eh.to_bits();
    let mut eh = eh;
    let near =
        (!exact && (lh & 0xfff_ffff) == 0) || (exact && (lh.wrapping_add(1) & 0xfff_ffff) <= 2);
    // |el| > 2^-91
    if near && el.abs() > f64::from_bits(0x3a40_0000_0000_0000) {
        if el < 0.0 {
            lh = lh.wrapping_sub(1);
        } else {
            lh = lh.wrapping_add(1);
        }
        eh = f64::from_bits(lh);
    }
    eh *= r;
    if xsgn && isodd(y0) {
        eh = -eh;
    }
    eh as f32
}

#[cfg(test)]
mod tests {
    use super::powf;

    #[test]
    fn special_and_exact_cases() {
        assert_eq!(powf(2.0, 10.0), 1024.0);
        assert_eq!(powf(4.0, 0.5), 2.0);
        assert_eq!(powf(8.0, -1.0 / 3.0f32), powf(8.0, -1.0 / 3.0f32)); // deterministic
        assert_eq!(powf(9.0, 0.5), 3.0);
        assert_eq!(powf(-2.0, 3.0), -8.0);
        assert!(powf(-2.0, 0.5).is_nan());
        assert_eq!(powf(0.0, -1.0), f32::INFINITY);
        assert_eq!(powf(-0.0, -1.0), f32::NEG_INFINITY);
        assert_eq!(powf(-0.0, 3.0).to_bits(), (-0.0f32).to_bits());
        assert_eq!(powf(1.0, f32::NAN), 1.0);
        assert_eq!(powf(f32::NAN, 0.0), 1.0);
        assert_eq!(powf(0.5, f32::INFINITY), 0.0);
        assert_eq!(powf(2.0, f32::INFINITY), f32::INFINITY);
        assert_eq!(powf(10.0, 40.0), f32::INFINITY);
        assert_eq!(powf(10.0, -50.0), 0.0);
        assert_eq!(powf(f32::INFINITY, -2.0), 0.0);
        assert_eq!(powf(f32::NEG_INFINITY, 3.0), f32::NEG_INFINITY);
        assert_eq!(powf(3.0, 2.0), 9.0);
        assert_eq!(powf(1.5, 2.0), 2.25);
    }

    #[test]
    fn close_to_f64_power() {
        let mut s = 0x9e37_79b9_7f4a_7c15u64;
        for _ in 0..100_000 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let x = (s % 1_000_000) as f32 / 997.0 + 0.001;
            let y = ((s >> 20) % 4000) as f32 / 200.0 - 10.0;
            let r = powf(x, y);
            let reference = f64::from(x).powf(f64::from(y));
            if reference.is_finite() && reference > 1e-37 && reference < 1e37 {
                let rel = (f64::from(r) - reference).abs() / reference;
                assert!(rel < 6.0e-8, "{x} ^ {y} = {r}, expected about {reference}");
            }
        }
    }
}
