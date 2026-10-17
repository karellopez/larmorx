// SPDX-License-Identifier: Apache-2.0 AND MIT
//! Correctly rounded arcsine for binary64 (`asin`).
//!
//! Port of CORE-MATH `src/binary64/asin/asin.c` at commit 040ee482a8ca
//! (<https://core-math.gitlabpages.inria.fr/>), re-expressed in Rust.
//! Round-to-nearest only: the errno code of the C file is dropped. Algorithms, tables and the
//! order of every floating-point operation are kept exactly; `__builtin_fma` is
//! [`f64::mul_add`] (correctly rounded on every target), `roundeven_finite` is
//! [`f64::round_ties_even`], and hex-float constants are written as their exact binary64 bit
//! patterns.
//!
//! Original licence of asin.c (MIT):
//!
//! Copyright (c) 2023-2026 Alexei Sibidanov.
//!
//! This file is part of the CORE-MATH project
//! (<https://core-math.gitlabpages.inria.fr/>).
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy
//! of this software and associated documentation files (the "Software"), to deal
//! in the Software without restriction, including without limitation the rights
//! to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//! copies of the Software, and to permit persons to whom the Software is
//! furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in all
//! copies or substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//! IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//! FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//! AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//! OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
//! SOFTWARE.

#![allow(clippy::excessive_precision, clippy::unreadable_literal)]

/// `f64` from its IEEE-754 bit pattern (tables are stored as exact bit patterns).
const fn fb(bits: u64) -> f64 {
    f64::from_bits(bits)
}

#[inline(always)]
fn fasttwosum(x: f64, y: f64, e: &mut f64) -> f64 {
    let s = x + y;
    let z = s - x;
    *e = y - z;
    s
}

#[inline(always)]
fn fastsum(xh: f64, xl: f64, yh: f64, yl: f64, e: &mut f64) -> f64 {
    let mut sl = 0.0;
    let sh = fasttwosum(xh, yh, &mut sl);
    *e = (xl + yl) + sl;
    sh
}

#[inline(always)]
fn muldd(xh: f64, xl: f64, ch: f64, cl: f64, l: &mut f64) -> f64 {
    let ahhh = ch * xh;
    *l = (cl * xh + ch * xl) + ch.mul_add(xh, -ahhh);
    ahhh
}

#[inline(always)]
fn polydd(xh: f64, xl: f64, n: usize, c: &[[f64; 2]], l: &mut f64) -> f64 {
    let mut i = n - 1;
    let mut ll = 0.0;
    let mut ch = fasttwosum(c[i][0], *l, &mut ll);
    let mut cl = c[i][1] + ll;
    while i > 0 {
        i -= 1;
        ch = muldd(xh, xl, ch, cl, &mut cl);
        ch = fastsum(c[i][0], c[i][1], ch, cl, &mut cl);
    }
    *l = cl;
    ch
}

/// `cr_asin`.
#[inline(always)]
pub(crate) fn asin(x: f64) -> f64 {
    let ixu = x.to_bits();
    let ax = ixu << 1;
    let (mut t, z, zl, jd, f0h, f0l, eps);
    if ax > 0x7fc0000000000000 {
        // |x| > 0.5
        let k = (ixu >> 63) as usize;
        f0h = OFF[k][0];
        f0l = OFF[k][1];
        if ax >= 0x7fe0000000000000 {
            // |x| >= 1
            if ax == 0x7fe0000000000000 {
                return f0h + f0l; // |x| = 1
            }
            if ax > 0xffe0000000000000 {
                return x + x; // nan
            }
            return f64::NAN; // |x| > 1
        }
        // asin(x) = pi/2 - 2*asin(sqrt((1-x)/2)) for |x| > 0.5
        t = 2.0 - 2.0 * x.abs();
        jd = (t * fb(0x4040000000000000)).round_ties_even();
        z = t.sqrt().copysign(-x);
        zl = z.mul_add(z, -t) * ((-0.5 / t) * z);
        t = 0.25 * t - jd * fb(0x3f80000000000000);
        eps = (z * t).abs() * fb(0x3cb9900000000000);
    } else {
        // |x| <= 0.5
        if ax < 0x7cae26e892247dec {
            return fb(0x3c80000000000000).mul_add(x, x);
        }
        f0h = 0.0;
        f0l = 0.0;
        t = x * x;
        jd = (t * fb(0x4060000000000000)).round_ties_even();
        t = x.mul_add(x, fb(0xbf80000000000000) * jd);
        z = x;
        zl = 0.0;
        eps = (z * t).abs() * fb(0x3cb7700000000000);
    }
    let j = jd as i64 as usize;
    let c = &CC[j];
    let t2 = t * t;
    let d = t * ((c[2] + t * c[3]) + t2 * ((c[4] + t * c[5]) + t2 * (c[6] + t * c[7])));
    let mut fh = c[0];
    let mut fl = c[1] + d;
    fh = muldd(z, zl, fh, fl, &mut fl);
    fh = fastsum(f0h, f0l, fh, fl, &mut fl);
    let lb = fh + (fl - eps);
    let ub = fh + (fl + eps);
    if lb != ub {
        return as_asin_refine(x, lb);
    }
    lb
}

#[inline(always)]
fn as_asin_refine(x: f64, phi: f64) -> f64 {
    let s2 = x * x;
    let dx2 = x.mul_add(x, -s2);
    let mut c2l = 0.0;
    let mut c2h = fasttwosum(1.0, -s2, &mut c2l);
    c2l -= dx2;
    c2h = fasttwosum(c2h, c2l, &mut c2l);
    let ch = c2h.sqrt();
    let cl = (c2l - ch.mul_add(ch, -c2h)) * (0.5 / ch);
    let jf = (phi.abs() * fb(0x40345f306dc9c883)).round_ties_even() as i64 as usize;
    let (cc_h, cc_l, sh, sl) = (S[32 - jf][1], S[32 - jf][0], S[jf][1], S[jf][0]);
    let ax = x.abs();
    let dsh = ax - sh;
    let dsl = -sl;
    let dch = ch - cc_h;
    let dcl = cl - cc_l;
    const MAGIC: f64 = fb(0x3fb8000000000000);
    let sc = sh.mul_add(dch, MAGIC) - MAGIC;
    let dsc = sh.mul_add(dch, -sc);
    let cs = cc_h.mul_add(dsh, MAGIC) - MAGIC;
    let dcs = cc_h.mul_add(dsh, -cs);
    let mut v = cs - sc;
    let mut dv = (cc_h * dsl + cc_l * dsh) - (sh * dcl + sl * dch) - (dsc - dcs);
    v = fasttwosum(v, dv, &mut dv);
    let sgn = 1.0f64.copysign(x);
    let jt = jf as f64 * sgn;
    let mut dv2 = 0.0;
    let v2 = muldd(v, dv, v, dv, &mut dv2);
    v *= sgn;
    dv *= sgn;
    let mut fl = v2 * (CT[0] + v2 * (CT[1] + v2 * CT[2]));
    let mut fh = polydd(v2, dv2, 5, &C, &mut fl);
    fh = muldd(v, dv, fh, fl, &mut fl);
    let mut ph = jt * fb(0x3fa921fb54442d00);
    let mut pl = fb(0x3ca8469898cc5180) * jt;
    let mut ps = fb(0xb99fc8f8cbb5bf6c) * jt;
    pl = fastsum(fh, fl, pl, ps, &mut ps);
    ph = fasttwosum(ph, pl, &mut pl);
    pl = fasttwosum(pl, ps, &mut ps);
    ph = fasttwosum(ph, pl, &mut pl);
    pl = fasttwosum(pl, ps, &mut ps);
    let _ = ps;
    let th = ph.to_bits();
    let tl = pl.to_bits() & (!0u64 >> 1);
    let tn = (th & (0x7ffu64 << 52)).wrapping_sub(53u64 << 52);
    let dn = tl.wrapping_sub(tn) as i64;
    let de = (tn.wrapping_sub(tl) >> 52) as i64;
    let hard = (-2..=0).contains(&dn) || de > 47;
    let res = ph + pl;
    if hard {
        return as_asin_database(x, res);
    }
    res
}

fn as_asin_database(x: f64, f: f64) -> f64 {
    let ax = x.abs();
    let (mut a, mut b) = (0usize, DB.len());
    while a + 1 < b {
        let m = (a + b) / 2;
        if DB[m][0] <= ax {
            a = m;
        } else {
            b = m;
        }
    }
    if DB[a][0] == ax {
        if x > 0.0 {
            DB[a][1] + DB[a][2]
        } else {
            -DB[a][1] - DB[a][2]
        }
    } else {
        f
    }
}

/// `cc` of asin.c: per `j = round(128·x²)`, asin(x)/x ≈ c0 + c1 + t·P(t).
#[rustfmt::skip]
const CC: [[f64; 8]; 33] = [ [ 1.0, 0.0, fb(0x3fc5555555555555), fb(0x3fb33333333333e4), fb(0x3fa6db6db6d31f82), fb(0x3f9f1c71f6889397), fb(0x3f96e874b7045b46), fb(0x3f91f753132271e2)], [fb(0x3ff0055a27e0d033), fb(0xbc9d9ba10494c062), fb(0x3fc57c00cb5d6c4d), fb(0x3fb37881f5649a75), fb(0x3fa759af49d494dd), fb(0x3fa002e1864dda2e), fb(0x3f97c2d5d468cdd9), fb(0x3f9292834c025357)], [fb(0x3ff00abe0c129e1e), fb(0x3c67ceb0ee49d42a), fb(0x3fc5a3385d5c7ba5), fb(0x3fb3bf51056f6637), fb(0x3fa7dba76b124b37), fb(0x3fa07be4b02e94c4), fb(0x3f98a6bb92513f01), fb(0x3f936afd4c615aec)], [fb(0x3ff0102bcffd6acd), fb(0xbc8c2294c65d2e86), fb(0x3fc5caff17351901), fb(0x3fb407abbc04feb3), fb(0x3fa86179b8005949), fb(0x3fa0f97520bd4e72), fb(0x3f99950c5c89f3df), fb(0x3f944f2344e7b664)], [fb(0x3ff015a397cf0f1c), fb(0xbc8eebd6ccfe3ee3), fb(0x3fc5f3581be7b08b), fb(0x3fb4519ddf1ae531), fb(0x3fa8eb4b6ee35e92), fb(0x3fa17bc85414cd46), fb(0x3f9a8e5895e3fcf9), fb(0x3f953fafdc629400)], [fb(0x3ff01b2588811eeb), fb(0x3c47193e5d0a915f), fb(0x3fc61c46a67205d2), fb(0x3fb49d33a6eeae0b), fb(0x3fa979438563c014), fb(0x3fa20316ae977f05), fb(0x3f9b9339afb53aa4), fb(0x3f963d6b02c42d0a)], [fb(0x3ff020b1c7df0575), fb(0xbc8dd547e329c1e5), fb(0x3fc645ce0ab901bb), fb(0x3fb4ea79c34fc7a6), fb(0x3faa0b8ac08940ec), fb(0x3fa28f9babd0629b), fb(0x3f9ca452cf90a55e), fb(0x3f97492b016730ef)], [fb(0x3ff026487c8c5d71), fb(0xbc95fd9b68dc3b6e), fb(0x3fc66ff1b67d5d70), fb(0x3fb5397d613373eb), fb(0x3faaa24bce3aeb4a), fb(0x3fa3219610b150ac), fb(0x3f9dc251825103f1), fb(0x3f9863d5a3932532)], [fb(0x3ff02be9ce0b87cd), fb(0x3c7e5d09da2e0f04), fb(0x3fc69ab5325bc359), fb(0x3fb58a4c3097aab3), fb(0x3fab3db3605f46f2), fb(0x3fa3b94821742cab), fb(0x3f9eedee7da72a15), fb(0x3f998e6179a3e9a0)], [fb(0x3ff03195e4c483f1), fb(0xbc95db10ad66eacb), fb(0x3fc6c61c22d908f0), fb(0x3fb5dcf46ab9f2cb), fb(0x3fabddf049bb1f4d), fb(0x3fa456f7db6ac768), fb(0x3fa013f738bd7bb3), fb(0x3f9ac9d739783d21)], [fb(0x3ff0374cea0c0c9f), fb(0xbc9917bff5241c76), fb(0x3fc6f22a497b2ec0), fb(0x3fb63184d8a79db5), fb(0x3fac83339caf946e), fb(0x3fa4faef331019d4), fb(0x3fa0b8917547d678), fb(0x3f9c17533f147e1c)], [fb(0x3ff03d0f082afcc8), fb(0xbc9018bbcddb49eb), fb(0x3fc71ee385efdf06), fb(0x3fb6880cda2d3885), fb(0x3fad2db0cbfae54d), fb(0x3fa5a57c56b50c5e), fb(0x3fa16535a40098b2), fb(0x3f9d780730b8ebb8)], [fb(0x3ff042dc6a65ffbf), fb(0xbc8c7ea28dce95d1), fb(0x3fc74c4bd7412f9e), fb(0x3fb6e09c6d2b72bc), fb(0x3faddd9dcda253de), fb(0x3fa656f1f62b5001), fb(0x3fa21a5ae2ac77ee), fb(0x3f9eed3bca067f0e)], [fb(0x3ff048b53d05907b), fb(0x3c9634fffed6e2a6), fb(0x3fc77a675d1978be), fb(0x3fb73b4435583415), fb(0x3fae9333402ebbf3), fb(0x3fa70fa78fd9f73f), fb(0x3fa2d8804d934fe1), fb(0x3fa03c29691a281c)], [fb(0x3ff04e99ad5e4bcd), fb(0xbc9e97a72fe827e0), fb(0x3fc7a93a5917200c), fb(0x3fb7981584731c05), fb(0x3faf4eac9268fae2), fb(0x3fa7cff9c3b19721), fb(0x3fa3a02d9c1e0145), fb(0x3fa10d64a0e56953)], [fb(0x3ff05489e9d99995), fb(0x3c8d177637ec6a2b), fb(0x3fc7d8c930314681), fb(0x3fb7f72262f532e4), fb(0x3fb0082416e39013), fb(0x3fa8984aac80ddf4), fb(0x3fa471f3caf18eb8), fb(0x3fa1eb1cce6dd570)], [fb(0x3ff05a8621feb16b), fb(0xbc7e5b33b1407c5f), fb(0x3fc809186c2e57dd), fb(0x3fb8587d99442dc8), fb(0x3fb06c23d1dfcb7f), fb(0x3fa969024036dd22), fb(0x3fa54e6dd4d2af33), fb(0x3fa2d62f439f2a31)], [fb(0x3ff0608e867bff30), fb(0x3c8cbef5d8580027), fb(0x3fc83a2cbd2d8ba2), fb(0x3fb8bc3ab9724c6e), fb(0x3fb0d377ef1e0c39), fb(0x3faa428eb7addf84), fb(0x3fa636417bc01ff2), fb(0x3fa3cf8acc7eb2a0)], [fb(0x3ff066a34930ec8d), fb(0xbc9480f445fedad1), fb(0x3fc86c0afb447a74), fb(0x3fb9226e29948d9c), fb(0x3fb13e44a9b5a3a6), fb(0x3fab2564fea8b3fe), fb(0x3fa72a2023d92458), fb(0x3fa4d8313cec3485)], [fb(0x3ff06cc49d38146c), fb(0xbc8b55394f4fc07b), fb(0x3fc89eb82831feed), fb(0x3fb98b2d2eb9bb23), fb(0x3fb1acb01e9ab414), fb(0x3fac12012cbd00c6), fb(0x3fa82ac7c1d15c38), fb(0x3fa5f13925c6edca)], [fb(0x3ff072f2b6f1e601), fb(0xbc92dcbb05419970), fb(0x3fc8d2397127aebb), fb(0x3fb9f68df88da51d), fb(0x3fb21ee26a4f62a1), fb(0x3fad08e707f7ae6f), fb(0x3fa93903dee3feb0), fb(0x3fa71bcfb5c57b59)], [fb(0x3ff0792dcc0fbd20), fb(0xbc75bf23ee4f9d54), fb(0x3fc9069430ab508a), fb(0x3fba64a7adb4cd85), fb(0x3fb29505c8b48349), fb(0x3fae0aa2921cfa60), fb(0x3faa55aeb46f4322), fb(0x3fa8593acad3bece)], [fb(0x3ff07f76139f761d), fb(0x3c9fa1046481bb82), fb(0x3fc93bcdf091cca6), fb(0x3fbad59278edc42f), fb(0x3fb30f46b7261652), fb(0x3faf17c8a17c843e), fb(0x3fab81b2619e15b5), fb(0x3fa9aadb395f5ae4)], [fb(0x3ff085cbc61783c1), fb(0x3c90a6e9efa20176), fb(0x3fc971ec6c1531e4), fb(0x3fbb496797068912), fb(0x3fb38dd419140184), fb(0x3fb0187bc3357bbb), fb(0x3facbe0a3dcafe26), fb(0x3fab122f4fa499d0)], [fb(0x3ff08c2f1d638e4c), fb(0x3c7b47c159534a3d), fb(0x3fc9a8f592078624), fb(0x3fbbc04165b57ab2), fb(0x3fb410df5f4bed1d), fb(0x3fb0ab6bdf478c71), fb(0x3fae0bc44a945c64), fb(0x3fac90d59bcd5701)], [fb(0x3ff092a054f1a2fc), fb(0xbc92f657224e9830), fb(0x3fc9e0ef87243a2c), fb(0x3fbc3a3b7366a278), fb(0x3fb4989cb22e2175), fb(0x3fb1450e5ba7ad39), fb(0x3faf6c02c8f0ef93), fb(0x3fae288ffc8d182c)], [fb(0x3ff0991fa9bffbf4), fb(0xbc5ca1140a1abbf4), fb(0x3fca19e0a8823b80), fb(0x3fbcb772900f9c24), fb(0x3fb525431f0cbb2e), fb(0x3fb1e5c2d06804e1), fb(0x3fb06ffefa7aa6b8), fb(0x3fafdb4704dca347)], [fb(0x3ff09fad5a6b68f9), fb(0x3c7aa1f06e92964e), fb(0x3fca53cf8e28c50e), fb(0x3fbd3804df1de350), fb(0x3fb5b70cc8fa98dc), fb(0x3fb28def298c979b), fb(0x3fb13482f6347eeb), fb(0x3fb0d586de48358c)], [fb(0x3ff0a649a73e61f2), fb(0x3c874ac0d817e9c7), fb(0x3fca8ec30dc93891), fb(0x3fbdbc11ea950625), fb(0x3fb64e371d5616d3), fb(0x3fb33e0023936249), fb(0x3fb204426263066a), fb(0x3fb1cd12e4629723)], [fb(0x3ff0acf4d240ccc4), fb(0x3c9da890f3b40bd3), fb(0x3fcacac23da07797), fb(0x3fbe43bab7741a98), fb(0x3fb6eb030c631819), fb(0x3fb3f669d2eb516e), fb(0x3fb2e0006ae505ae), fb(0x3fb2d58204457c82)], [fb(0x3ff0b3af1f4880bb), fb(0x3c7f450fb78d32ba), fb(0x3fcb07d4778263af), fb(0x3fbecf21db7be0ef), fb(0x3fb78db5465013e4), fb(0x3fb4b7a8376f0996), fb(0x3fb3c88f9f2ef221), fb(0x3fb3f02ad9eb9753)], [fb(0x3ff0ba78d40a9260), fb(0xbc957b07a441e242), fb(0x3fcb46015c126262), fb(0x3fbf5e6b94713f3d), fb(0x3fb836967d0afecf), fb(0x3fb5823fdd1707b9), fb(0x3fb4bed355269dc2), fb(0x3fb51e83065121cf)], [fb(0x3ff0c152382d7366), fb(0xbc9ee6913347c2a6), fb(0x3fcb8550d62bfb6e), fb(0x3fbff1bde0fa3cad), fb(0x3fb8e5f3ab550989), fb(0x3fb656be8b38ebaf), fb(0x3fb5c3c13008a099), fb(0x3fb662225a1b4f77)], ];

/// `off` of asin.c: ±π/2 as double-doubles.
#[rustfmt::skip]
const OFF: [[f64; 2]; 2] = [ [fb(0x3ff921fb54442d18), fb(0x3c91a62633145c07)], [fb(0xbff921fb54442d18), fb(0xbc91a62633145c07)] ];

/// `s` of asin.c: sin(π·j/64), double-double (small term first).
#[rustfmt::skip]
const S: [[f64; 2]; 33] = [ [fb(0x0000000000000000), fb(0x0000000000000000)], [fb(0xbc2912bd0d569a90), fb(0x3fa91f65f10dd814)], [fb(0xbc3e2718d26ed688), fb(0x3fb917a6bc29b42c)], [fb(0x3c513000a89a11e0), fb(0x3fc2c8106e8e613a)], [fb(0xbc626d19b9ff8d82), fb(0x3fc8f8b83c69a60b)], [fb(0xbc642deef11da2c4), fb(0x3fcf19f97b215f1b)], [fb(0xbc75d28da2c4612d), fb(0x3fd294062ed59f06)], [fb(0xbc1efdc0d58cf620), fb(0x3fd58f9a75ab1fdd)], [fb(0xbc672cedd3d5a610), fb(0x3fd87de2a6aea963)], [fb(0x3c65b362cb974183), fb(0x3fdb5d1009e15cc0)], [fb(0x3c5e0d891d3c6841), fb(0x3fde2b5d3806f63b)], [fb(0xbc8a5a014347406c), fb(0x3fe073879922ffee)], [fb(0x3c8b25dd267f6600), fb(0x3fe1c73b39ae68c8)], [fb(0xbc6efcc626f74a6f), fb(0x3fe30ff7fce17035)], [fb(0x3c68076a2cfdc6b3), fb(0x3fe44cf325091dd6)], [fb(0xbc875720992bfbb2), fb(0x3fe57d69348ceca0)], [fb(0xbc8bdd3413b26456), fb(0x3fe6a09e667f3bcd)], [fb(0xbc70f537acdf0ad7), fb(0x3fe7b5df226aafaf)], [fb(0xbc82c5e12ed1336d), fb(0x3fe8bc806b151741)], [fb(0xbc830ee286712474), fb(0x3fe9b3e047f38741)], [fb(0x3c39f630e8b6dac8), fb(0x3fea9b66290ea1a3)], [fb(0xbc8bc69f324e6d61), fb(0x3feb728345196e3e)], [fb(0xbc76e0b1757c8d07), fb(0x3fec38b2f180bdb1)], [fb(0xbc5e7b6bb5ab58ae), fb(0x3feced7af43cc773)], [fb(0x3c7457e610231ac2), fb(0x3fed906bcf328d46)], [fb(0xbc8014c76c126527), fb(0x3fee212104f686e5)], [fb(0x3c8760b1e2e3f81e), fb(0x3fee9f4156c62dda)], [fb(0x3c752c7adc6b4989), fb(0x3fef0a7efb9230d7)], [fb(0x3c7562172a361fd3), fb(0x3fef6297cff75cb0)], [fb(0xbc87a0a8ca13571f), fb(0x3fefa7557f08a517)], [fb(0xbc887df6378811c7), fb(0x3fefd88da3d12526)], [fb(0xbc6c57bc2e24aa15), fb(0x3feff621e3796d7e)], [fb(0x0000000000000000), fb(0x3ff0000000000000)] ];

/// `c` of asin.c (as in acos.c).
#[rustfmt::skip]
const C: [[f64; 2]; 5] = [ [fb(0x3ff0000000000000), fb(0xb93fc2c76456515b)], [fb(0x3fc5555555555555), fb(0x3c65555555623513)], [fb(0x3fb3333333333333), fb(0x3c49997e3427441b)], [fb(0x3fa6db6db6db6db7), fb(0xbc1cb95ff08658e6)], [fb(0x3f9f1c71c71c6d5b), fb(0x3c3b125bccdcc89e)]];

/// `ct` of asin.c (as in acos.c).
#[rustfmt::skip]
const CT: [f64; 3] = [fb(0x3f96e8ba2ec8cb69), fb(0x3f91c4ea7a15c997), fb(0x3f8ca8355d39bb67)];

/// `db` of asin.c: the exceptional cases.
#[rustfmt::skip]
const DB: [[f64; 3]; 29] = [ [fb(0x3e57137449123ef6), fb(0x3e57137449123ef7), fb(0xbaffffffffffffff)], [fb(0x3e5d12ed0af1a27e), fb(0x3e5d12ed0af1a27f), fb(0xb7963676aa7969aa)], [fb(0x3e851c4b960778f5), fb(0x3e851c4b9607790d), fb(0x3b2fffffffffffff)], [fb(0x3e93cfc2a006a414), fb(0x3e93cfc2a006a465), fb(0xb7c1e68bc303367c)], [fb(0x3e9cbaa95dadb559), fb(0x3e9cbaa95dadb650), fb(0x37f742593ef4c3d3)], [fb(0x3ebacd69f89ad8f1), fb(0x3ebacd69f89ae57a), fb(0x381b0ddfe3e29b3c)], [fb(0x3ef2bffffffc233b), fb(0x3ef2c00000006ddd), fb(0xb851c26f2054c230)], [fb(0x3efff0f3022b2e9d), fb(0x3efff0f3024065e7), fb(0xbb9ffffffffffffe)], [fb(0x3f13217783d70d1d), fb(0x3f132177841ffbef), fb(0xbbbfffffffffffff)], [fb(0x3f1c373ff4aad79b), fb(0x3f1c373ff594d65b), fb(0xbbbfffffffffffff)], [fb(0x3f6b3f28593cad2f), fb(0x3f6b3f2ba40dbc66), fb(0xb8b27ca20c684481)], [fb(0x3f8e17b3f6bb5e6e), fb(0x3f8e17faefac7797), fb(0xb8f15a1c95184d1b)], [fb(0x3f941d60a76a82ed), fb(0x3f941db571d96126), fb(0xb8ddf34bcdacf353)], [fb(0x3f9921c0a0486537), fb(0x3f99226605233224), fb(0xb9338e672f1696d5)], [fb(0x3f99c360a8dd681a), fb(0x3f99c412d62c1440), fb(0x3911edd261167baf)], [fb(0x3f9d6315f7ee7e01), fb(0x3f9d641e6d5e769a), fb(0xb92af7dec023f5b9)], [fb(0x3f9ea6fdc56fc61a), fb(0x3f9ea829e3e988e5), fb(0xb92f358e90326f9a)], [fb(0x3fa2749dc4d19c6d), fb(0x3fa275a3d78c01ec), fb(0x39149ab71c17bd11)], [fb(0x3fa69768dc89bb00), fb(0x3fa69949b3d51fb1), fb(0xb8e14b91318650e8)], [fb(0x3faa4816b2066707), fb(0x3faa4b0bfb0454d5), fb(0x3c4fffffffffffff)], [fb(0x3fad77b117f230d6), fb(0x3fad7bdcd778049f), fb(0x38daafc31a59e093)], [fb(0x3fafc7a07b2549aa), fb(0x3fafccdc252cad1f), fb(0xb92dc02fdac5159a)], [fb(0x3fb2df0542154f1b), fb(0x3fb2e36813a98740), fb(0xb9363abaa885a4da)], [fb(0x3fb51cf5db1b1956), fb(0x3fb5231b416ba885), fb(0xb93e1e3f45d0bec3)], [fb(0x3fcd0ef799001ba9), fb(0x3fcd5064e6fe82c5), fb(0xb9284b9ecb11e241)], [fb(0x3fd4a8e1a96e38e3), fb(0x3fd50954b7bbf87b), fb(0xb93b9f88cfb1ef42)], [fb(0x3fdceee68154d1c9), fb(0x3fde05b0e0a809bc), fb(0x394117d6aecd2594)], [fb(0x3fdda4e0e6c717a5), fb(0x3fded25c5eb8c916), fb(0xb9289e120e5744a5)], [fb(0x3fde9950730c4696), fb(0x3fdfe767739d0f6d), fb(0x38716376667a73f3)], ];
