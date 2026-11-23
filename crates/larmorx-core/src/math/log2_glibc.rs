// SPDX-License-Identifier: Apache-2.0 AND MIT
// Translated to Rust from Arm's Optimized Routines v19.11 (github.com/ARM-software/optimized-routines,
// tag v19.11): math/log2.c and math/log2_data.c (__log2_data). Copyright (c) 2018, Arm Limited,
// under the MIT licence (its text and notice are in LICENSES/MIT-arm-optimized-routines.txt and
// NOTICE). Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the originals.
//! The base-2 logarithm for binary64 with the bits of glibc 2.35's `log2` on x86-64 (the current
//! symbol `log2@@GLIBC_2.29`), on every platform.
//!
//! glibc 2.35's x86-64 `log2` has one build, without fused multiply-adds: Arm's `log2.c` without
//! `HAVE_FAST_FMA` (the reduction through `tab2`'s split centres, `r`'s split into `rhi` and `rlo`),
//! every product and sum rounded as written. Measured as a black box, not read: Arm's v19.11 source
//! built that way (GCC 11.4, `-O2`, `HAVE_FAST_FMA` 0) equals the system's `log2` (glibc 2.35, an
//! x86-64 CPU with FMA and AVX2) on 402,653,199 arguments (random bit patterns, uniform on (0, 1000),
//! next to 1, subnormal and normal numbers of both signs, probabilities, special values), NaN bits
//! included, while the same source with `HAVE_FAST_FMA` and FMA code differs on 175,963 of them;
//! this translation is checked against the system's `log2` by `validation/probes/coremath-log2/`.
//! The NaN results are SSE's (`super::x86_nan`): the logarithm of a negative number or of `-inf`
//! is SSE's default NaN (`-nan`, `0xfff8...`), a NaN argument comes back quieted. Not correctly
//! rounded (Arm: 0.547 ulp); [`super::log2`] is, and is larmorx's default. This one serves
//! `reproduce` (FreeSurfer's joint-histogram costs, `mri_robust_register --cost MI|NMI|ECC`).

#![allow(clippy::unreadable_literal)]

use super::x86_nan::invalid_f64;

/// `__log2_data` (Arm's `log2_data.c`, `LOG2_TABLE_BITS` 6, `LOG2_POLY_ORDER` 7,
/// `LOG2_POLY1_ORDER` 11, without `HAVE_FAST_FMA`: with `tab2`), as 64-bit patterns transcribed
/// mechanically from the source's hexadecimal constants.
#[rustfmt::skip]
mod log2_data {
    /// `invln2hi` (0x1.7154765200000p+0).
    pub(super) const INVLN2HI: u64 = 0x3ff7154765200000;
    /// `invln2lo` (0x1.705fc2eefa200p-33).
    pub(super) const INVLN2LO: u64 = 0x3de705fc2eefa200;
    /// `poly1` (10 coefficients).
    pub(super) const POLY1: [u64; 10] = [
        0xbfe71547652b82fe, // -0x1.71547652b82fep-1
        0x3fdec709dc3a03f7, // 0x1.ec709dc3a03f7p-2
        0xbfd71547652b7c3f, // -0x1.71547652b7c3fp-2
        0x3fd2776c50f05be4, // 0x1.2776c50f05be4p-2
        0xbfcec709dd768fe5, // -0x1.ec709dd768fe5p-3
        0x3fca61761ec4e736, // 0x1.a61761ec4e736p-3
        0xbfc7153fbc64a79b, // -0x1.7153fbc64a79bp-3
        0x3fc484d154f01b4a, // 0x1.484d154f01b4ap-3
        0xbfc289e4a72c383c, // -0x1.289e4a72c383cp-3
        0x3fc0b32f285aee66, // 0x1.0b32f285aee66p-3
    ];
    /// `poly` (6 coefficients).
    pub(super) const POLY: [u64; 6] = [
        0xbfe71547652b8339, // -0x1.71547652b8339p-1
        0x3fdec709dc3a04be, // 0x1.ec709dc3a04bep-2
        0xbfd7154764702ffb, // -0x1.7154764702ffbp-2
        0x3fd2776c50034c48, // 0x1.2776c50034c48p-2
        0xbfcec7b328ea92bc, // -0x1.ec7b328ea92bcp-3
        0x3fca6225e117f92e, // 0x1.a6225e117f92ep-3
    ];
    /// `tab`: (invc, logc).
    pub(super) const TAB: [(u64, u64); 64] = [
        (0x3ff724286bb1acf8, 0xbfe1095feecdb000), // {0x1.724286bb1acf8p+0, -0x1.1095feecdb000p-1}
        (0x3ff6e1f766d2cca1, 0xbfe08494bd76d000), // {0x1.6e1f766d2cca1p+0, -0x1.08494bd76d000p-1}
        (0x3ff6a13d0e30d48a, 0xbfe00143aee8f800), // {0x1.6a13d0e30d48ap+0, -0x1.00143aee8f800p-1}
        (0x3ff661ec32d06c85, 0xbfdefec5360b4000), // {0x1.661ec32d06c85p+0, -0x1.efec5360b4000p-2}
        (0x3ff623fa951198f8, 0xbfddfdd91ab7e000), // {0x1.623fa951198f8p+0, -0x1.dfdd91ab7e000p-2}
        (0x3ff5e75ba4cf026c, 0xbfdcffae0cc79000), // {0x1.5e75ba4cf026cp+0, -0x1.cffae0cc79000p-2}
        (0x3ff5ac055a214fb8, 0xbfdc043811fda000), // {0x1.5ac055a214fb8p+0, -0x1.c043811fda000p-2}
        (0x3ff571ed0f166e1e, 0xbfdb0b67323ae000), // {0x1.571ed0f166e1ep+0, -0x1.b0b67323ae000p-2}
        (0x3ff53909590bf835, 0xbfda152f5a2db000), // {0x1.53909590bf835p+0, -0x1.a152f5a2db000p-2}
        (0x3ff5014fed61addd, 0xbfd9217f5af86000), // {0x1.5014fed61adddp+0, -0x1.9217f5af86000p-2}
        (0x3ff4cab88e487bd0, 0xbfd8304db0719000), // {0x1.4cab88e487bd0p+0, -0x1.8304db0719000p-2}
        (0x3ff49539b4334fee, 0xbfd74189f9a9e000), // {0x1.49539b4334feep+0, -0x1.74189f9a9e000p-2}
        (0x3ff460cbdfafd569, 0xbfd6552bb5199000), // {0x1.460cbdfafd569p+0, -0x1.6552bb5199000p-2}
        (0x3ff42d664ee4b953, 0xbfd56b23a29b1000), // {0x1.42d664ee4b953p+0, -0x1.56b23a29b1000p-2}
        (0x3ff3fb01111dd8a6, 0xbfd483650f5fa000), // {0x1.3fb01111dd8a6p+0, -0x1.483650f5fa000p-2}
        (0x3ff3c995b70c5836, 0xbfd39de937f6a000), // {0x1.3c995b70c5836p+0, -0x1.39de937f6a000p-2}
        (0x3ff3991c4ab6fd4a, 0xbfd2baa1538d6000), // {0x1.3991c4ab6fd4ap+0, -0x1.2baa1538d6000p-2}
        (0x3ff3698e0ce099b5, 0xbfd1d98340ca4000), // {0x1.3698e0ce099b5p+0, -0x1.1d98340ca4000p-2}
        (0x3ff33ae48213e7b2, 0xbfd0fa853a40e000), // {0x1.33ae48213e7b2p+0, -0x1.0fa853a40e000p-2}
        (0x3ff30d191985bdb1, 0xbfd01d9c32e73000), // {0x1.30d191985bdb1p+0, -0x1.01d9c32e73000p-2}
        (0x3ff2e025cab271d7, 0xbfce857da2fa6000), // {0x1.2e025cab271d7p+0, -0x1.e857da2fa6000p-3}
        (0x3ff2b404cf13cd82, 0xbfccd3c8633d8000), // {0x1.2b404cf13cd82p+0, -0x1.cd3c8633d8000p-3}
        (0x3ff288b02c7ccb50, 0xbfcb26034c14a000), // {0x1.288b02c7ccb50p+0, -0x1.b26034c14a000p-3}
        (0x3ff25e2263944de5, 0xbfc97c1c2f4fe000), // {0x1.25e2263944de5p+0, -0x1.97c1c2f4fe000p-3}
        (0x3ff234563d8615b1, 0xbfc7d6023f800000), // {0x1.234563d8615b1p+0, -0x1.7d6023f800000p-3}
        (0x3ff20b46e33eaf38, 0xbfc633a71a05e000), // {0x1.20b46e33eaf38p+0, -0x1.633a71a05e000p-3}
        (0x3ff1e2eefdcda3dd, 0xbfc494f5e9570000), // {0x1.1e2eefdcda3ddp+0, -0x1.494f5e9570000p-3}
        (0x3ff1bb4a580b3930, 0xbfc2f9e424e0a000), // {0x1.1bb4a580b3930p+0, -0x1.2f9e424e0a000p-3}
        (0x3ff19453847f2200, 0xbfc162595afdc000), // {0x1.19453847f2200p+0, -0x1.162595afdc000p-3}
        (0x3ff16e06c0d5d73c, 0xbfbf9c9a75bd8000), // {0x1.16e06c0d5d73cp+0, -0x1.f9c9a75bd8000p-4}
        (0x3ff1485f47b7e4c2, 0xbfbc7b575bf9c000), // {0x1.1485f47b7e4c2p+0, -0x1.c7b575bf9c000p-4}
        (0x3ff12358ad0085d1, 0xbfb960c60ff48000), // {0x1.12358ad0085d1p+0, -0x1.960c60ff48000p-4}
        (0x3ff0fef00f532227, 0xbfb64ce247b60000), // {0x1.0fef00f532227p+0, -0x1.64ce247b60000p-4}
        (0x3ff0db2077d03a8f, 0xbfb33f78b2014000), // {0x1.0db2077d03a8fp+0, -0x1.33f78b2014000p-4}
        (0x3ff0b7e6d65980d9, 0xbfb0387d1a42c000), // {0x1.0b7e6d65980d9p+0, -0x1.0387d1a42c000p-4}
        (0x3ff0953efe7b408d, 0xbfaa6f9208b50000), // {0x1.0953efe7b408dp+0, -0x1.a6f9208b50000p-5}
        (0x3ff07325cac53b83, 0xbfa47a954f770000), // {0x1.07325cac53b83p+0, -0x1.47a954f770000p-5}
        (0x3ff05197e40d1b5c, 0xbf9d23a8c50c0000), // {0x1.05197e40d1b5cp+0, -0x1.d23a8c50c0000p-6}
        (0x3ff03091c1208ea2, 0xbf916a2629780000), // {0x1.03091c1208ea2p+0, -0x1.16a2629780000p-6}
        (0x3ff0101025b37e21, 0xbf7720f8d8e80000), // {0x1.0101025b37e21p+0, -0x1.720f8d8e80000p-8}
        (0x3fefc07ef9caa76b, 0x3f86fe53b1500000), // {0x1.fc07ef9caa76bp-1, 0x1.6fe53b1500000p-7}
        (0x3fef4465d3f6f184, 0x3fa11ccce10f8000), // {0x1.f4465d3f6f184p-1, 0x1.11ccce10f8000p-5}
        (0x3feecc079f84107f, 0x3fac4dfc8c8b8000), // {0x1.ecc079f84107fp-1, 0x1.c4dfc8c8b8000p-5}
        (0x3fee573a99975ae8, 0x3fb3aa321e574000), // {0x1.e573a99975ae8p-1, 0x1.3aa321e574000p-4}
        (0x3fede5d6f0bd3de6, 0x3fb918a0d08b8000), // {0x1.de5d6f0bd3de6p-1, 0x1.918a0d08b8000p-4}
        (0x3fed77b681ff38b3, 0x3fbe72e9da044000), // {0x1.d77b681ff38b3p-1, 0x1.e72e9da044000p-4}
        (0x3fed0cb5724de943, 0x3fc1dcd2507f6000), // {0x1.d0cb5724de943p-1, 0x1.1dcd2507f6000p-3}
        (0x3feca4b2dc0e7563, 0x3fc476ab03dea000), // {0x1.ca4b2dc0e7563p-1, 0x1.476ab03dea000p-3}
        (0x3fec3f8ee8d6cb51, 0x3fc7074377e22000), // {0x1.c3f8ee8d6cb51p-1, 0x1.7074377e22000p-3}
        (0x3febdd2b4f020c4c, 0x3fc98ede8ba94000), // {0x1.bdd2b4f020c4cp-1, 0x1.98ede8ba94000p-3}
        (0x3feb7d6c006015ca, 0x3fcc0db86ad2e000), // {0x1.b7d6c006015cap-1, 0x1.c0db86ad2e000p-3}
        (0x3feb20366e2e338f, 0x3fce840aafcee000), // {0x1.b20366e2e338fp-1, 0x1.e840aafcee000p-3}
        (0x3feac57026295039, 0x3fd0790ab4678000), // {0x1.ac57026295039p-1, 0x1.0790ab4678000p-2}
        (0x3fea6d01bc2731dd, 0x3fd1ac056801c000), // {0x1.a6d01bc2731ddp-1, 0x1.1ac056801c000p-2}
        (0x3fea16d3bc3ff18b, 0x3fd2db11d4fee000), // {0x1.a16d3bc3ff18bp-1, 0x1.2db11d4fee000p-2}
        (0x3fe9c2d14967fead, 0x3fd406464ec58000), // {0x1.9c2d14967feadp-1, 0x1.406464ec58000p-2}
        (0x3fe970e4f47c9902, 0x3fd52dbe093af000), // {0x1.970e4f47c9902p-1, 0x1.52dbe093af000p-2}
        (0x3fe920fb3982bcf2, 0x3fd651902050d000), // {0x1.920fb3982bcf2p-1, 0x1.651902050d000p-2}
        (0x3fe8d30187f759f1, 0x3fd771d2cdeaf000), // {0x1.8d30187f759f1p-1, 0x1.771d2cdeaf000p-2}
        (0x3fe886e5ebb9f66d, 0x3fd88e9c857d9000), // {0x1.886e5ebb9f66dp-1, 0x1.88e9c857d9000p-2}
        (0x3fe83c97b658b994, 0x3fd9a80155e16000), // {0x1.83c97b658b994p-1, 0x1.9a80155e16000p-2}
        (0x3fe7f405ffc61022, 0x3fdabe186ed3d000), // {0x1.7f405ffc61022p-1, 0x1.abe186ed3d000p-2}
        (0x3fe7ad22181415ca, 0x3fdbd0f2aea0e000), // {0x1.7ad22181415cap-1, 0x1.bd0f2aea0e000p-2}
        (0x3fe767dcf99eff8c, 0x3fdce0a43dbf4000), // {0x1.767dcf99eff8cp-1, 0x1.ce0a43dbf4000p-2}
    ];
    /// `tab2`: (chi, clo).
    pub(super) const TAB2: [(u64, u64); 64] = [
        (0x3fe6200012b90a8e, 0x3c8904ab0644b605), // {0x1.6200012b90a8ep-1, 0x1.904ab0644b605p-55}
        (0x3fe66000045734a6, 0x3c61ff9bea62f7a9), // {0x1.66000045734a6p-1, 0x1.1ff9bea62f7a9p-57}
        (0x3fe69fffc325f2c5, 0x3c827ecfcb3c90ba), // {0x1.69fffc325f2c5p-1, 0x1.27ecfcb3c90bap-55}
        (0x3fe6e00038b95a04, 0x3c88ff8856739326), // {0x1.6e00038b95a04p-1, 0x1.8ff8856739326p-55}
        (0x3fe71fffe09994e3, 0x3c8afd40275f82b1), // {0x1.71fffe09994e3p-1, 0x1.afd40275f82b1p-55}
        (0x3fe7600015590e10, 0xbc72fd75b4238341), // {0x1.7600015590e1p-1, -0x1.2fd75b4238341p-56}
        (0x3fe7a00012655bd5, 0x3c7808e67c242b76), // {0x1.7a00012655bd5p-1, 0x1.808e67c242b76p-56}
        (0x3fe7e0003259e9a6, 0xbc6208e426f622b7), // {0x1.7e0003259e9a6p-1, -0x1.208e426f622b7p-57}
        (0x3fe81fffedb4b2d2, 0xbc8402461ea5c92f), // {0x1.81fffedb4b2d2p-1, -0x1.402461ea5c92fp-55}
        (0x3fe860002dfafcc3, 0x3c6df7f4a2f29a1f), // {0x1.860002dfafcc3p-1, 0x1.df7f4a2f29a1fp-57}
        (0x3fe89ffff78c6b50, 0xbc8e0453094995fd), // {0x1.89ffff78c6b5p-1, -0x1.e0453094995fdp-55}
        (0x3fe8e00039671566, 0xbc8a04f3bec77b45), // {0x1.8e00039671566p-1, -0x1.a04f3bec77b45p-55}
        (0x3fe91fffe2bf1745, 0xbc77fa34400e203c), // {0x1.91fffe2bf1745p-1, -0x1.7fa34400e203cp-56}
        (0x3fe95fffcc5c9fd1, 0xbc76ff8005a0695d), // {0x1.95fffcc5c9fd1p-1, -0x1.6ff8005a0695dp-56}
        (0x3fe9a0003bba4767, 0x3c70f8c4c4ec7e03), // {0x1.9a0003bba4767p-1, 0x1.0f8c4c4ec7e03p-56}
        (0x3fe9dfffe7b92da5, 0x3c8e7fd9478c4602), // {0x1.9dfffe7b92da5p-1, 0x1.e7fd9478c4602p-55}
        (0x3fea1fffd72efdaf, 0xbc6a0c554dcdae7e), // {0x1.a1fffd72efdafp-1, -0x1.a0c554dcdae7ep-57}
        (0x3fea5fffde04ff95, 0x3c867da98ce9b26b), // {0x1.a5fffde04ff95p-1, 0x1.67da98ce9b26bp-55}
        (0x3fea9fffca5e8d2b, 0xbc8284c9b54c13de), // {0x1.a9fffca5e8d2bp-1, -0x1.284c9b54c13dep-55}
        (0x3feadfffddad03ea, 0x3c5812c8ea602e3c), // {0x1.adfffddad03eap-1, 0x1.812c8ea602e3cp-58}
        (0x3feb1ffff10d3d4d, 0xbc8efaddad27789c), // {0x1.b1ffff10d3d4dp-1, -0x1.efaddad27789cp-55}
        (0x3feb5fffce21165a, 0x3c53cb1719c61237), // {0x1.b5fffce21165ap-1, 0x1.3cb1719c61237p-58}
        (0x3feb9fffd950e674, 0x3c73f7d94194ce00), // {0x1.b9fffd950e674p-1, 0x1.3f7d94194cep-56}
        (0x3febe000139ca8af, 0x3c750ac4215d9bc0), // {0x1.be000139ca8afp-1, 0x1.50ac4215d9bcp-56}
        (0x3fec20005b46df99, 0x3c6beea653e9c1c9), // {0x1.c20005b46df99p-1, 0x1.beea653e9c1c9p-57}
        (0x3fec600040b9f7ae, 0xbc7c079f274a70d6), // {0x1.c600040b9f7aep-1, -0x1.c079f274a70d6p-56}
        (0x3feca0006255fd8a, 0xbc7a0b4076e84c1f), // {0x1.ca0006255fd8ap-1, -0x1.a0b4076e84c1fp-56}
        (0x3fecdfffd94c095d, 0x3c88f933f99ab5d7), // {0x1.cdfffd94c095dp-1, 0x1.8f933f99ab5d7p-55}
        (0x3fed1ffff975d6cf, 0xbc582c08665fe1be), // {0x1.d1ffff975d6cfp-1, -0x1.82c08665fe1bep-58}
        (0x3fed5fffa2561c93, 0xbc7b04289bd295f3), // {0x1.d5fffa2561c93p-1, -0x1.b04289bd295f3p-56}
        (0x3fed9fff9d228b0c, 0x3c870251340fa236), // {0x1.d9fff9d228b0cp-1, 0x1.70251340fa236p-55}
        (0x3fede00065bc7e16, 0xbc75011e16a4d80c), // {0x1.de00065bc7e16p-1, -0x1.5011e16a4d80cp-56}
        (0x3fee200002f64791, 0x3c89802f09ef62e0), // {0x1.e200002f64791p-1, 0x1.9802f09ef62ep-55}
        (0x3fee600057d7a6d8, 0xbc7e0b75580cf7fa), // {0x1.e600057d7a6d8p-1, -0x1.e0b75580cf7fap-56}
        (0x3feea00027edc00c, 0xbc8c848309459811), // {0x1.ea00027edc00cp-1, -0x1.c848309459811p-55}
        (0x3feee0006cf5cb7c, 0xbc8f8027951576f4), // {0x1.ee0006cf5cb7cp-1, -0x1.f8027951576f4p-55}
        (0x3fef2000782b7dcc, 0xbc8f81d97274538f), // {0x1.f2000782b7dccp-1, -0x1.f81d97274538fp-55}
        (0x3fef6000260c450a, 0xbc4071002727ffdc), // {0x1.f6000260c450ap-1, -0x1.071002727ffdcp-59}
        (0x3fef9fffe88cd533, 0xbc581bdce1fda8b0), // {0x1.f9fffe88cd533p-1, -0x1.81bdce1fda8bp-58}
        (0x3fefdfffd50f8689, 0x3c87f91acb918e6e), // {0x1.fdfffd50f8689p-1, 0x1.7f91acb918e6ep-55}
        (0x3ff0200004292367, 0x3c9b7ff365324681), // {0x1.0200004292367p+0, 0x1.b7ff365324681p-54}
        (0x3ff05fffe3e3d668, 0x3c86fa08ddae957b), // {0x1.05fffe3e3d668p+0, 0x1.6fa08ddae957bp-55}
        (0x3ff0a0000a85a757, 0xbc57e2de80d3fb91), // {0x1.0a0000a85a757p+0, -0x1.7e2de80d3fb91p-58}
        (0x3ff0e0001a5f3fcc, 0xbc91823305c5f014), // {0x1.0e0001a5f3fccp+0, -0x1.1823305c5f014p-54}
        (0x3ff11ffff8afbaf5, 0xbc8bfabb6680bac2), // {0x1.11ffff8afbaf5p+0, -0x1.bfabb6680bac2p-55}
        (0x3ff15fffe54d91ad, 0xbc9d7f121737e7ef), // {0x1.15fffe54d91adp+0, -0x1.d7f121737e7efp-54}
        (0x3ff1a00011ac36e1, 0x3c9c000a0516f5ff), // {0x1.1a00011ac36e1p+0, 0x1.c000a0516f5ffp-54}
        (0x3ff1e00019c84248, 0xbc9082fbe4da5da0), // {0x1.1e00019c84248p+0, -0x1.082fbe4da5dap-54}
        (0x3ff220000ffe5e6e, 0xbc88fdd04c9cfb43), // {0x1.220000ffe5e6ep+0, -0x1.8fdd04c9cfb43p-55}
        (0x3ff26000269fd891, 0x3c8cfe2a7994d182), // {0x1.26000269fd891p+0, 0x1.cfe2a7994d182p-55}
        (0x3ff2a00029a6e6da, 0xbc700273715e8bc5), // {0x1.2a00029a6e6dap+0, -0x1.00273715e8bc5p-56}
        (0x3ff2dfffe0293e39, 0x3c9b7c39dab2a6f9), // {0x1.2dfffe0293e39p+0, 0x1.b7c39dab2a6f9p-54}
        (0x3ff31ffff7dcf082, 0x3c7df1336edc5254), // {0x1.31ffff7dcf082p+0, 0x1.df1336edc5254p-56}
        (0x3ff35ffff05a8b60, 0xbc9e03564ccd31eb), // {0x1.35ffff05a8b6p+0, -0x1.e03564ccd31ebp-54}
        (0x3ff3a0002e0eaecc, 0x3c75f0e74bd3a477), // {0x1.3a0002e0eaeccp+0, 0x1.5f0e74bd3a477p-56}
        (0x3ff3e000043bb236, 0x3c9c7dcb149d8833), // {0x1.3e000043bb236p+0, 0x1.c7dcb149d8833p-54}
        (0x3ff4200002d187ff, 0x3c7e08afcf2d3d28), // {0x1.4200002d187ffp+0, 0x1.e08afcf2d3d28p-56}
        (0x3ff460000d387cb1, 0x3c820837856599a6), // {0x1.460000d387cb1p+0, 0x1.20837856599a6p-55}
        (0x3ff4a00004569f89, 0xbc89fa5c904fbcd2), // {0x1.4a00004569f89p+0, -0x1.9fa5c904fbcd2p-55}
        (0x3ff4e000043543f3, 0xbc781125ed175329), // {0x1.4e000043543f3p+0, -0x1.81125ed175329p-56}
        (0x3ff51fffcc027f0f, 0x3c9883d8847754dc), // {0x1.51fffcc027f0fp+0, 0x1.883d8847754dcp-54}
        (0x3ff55ffffd87b36f, 0xbc8709e731d02807), // {0x1.55ffffd87b36fp+0, -0x1.709e731d02807p-55}
        (0x3ff59ffff21df7ba, 0x3c87f79f68727b02), // {0x1.59ffff21df7bap+0, 0x1.7f79f68727b02p-55}
        (0x3ff5dfffebfc3481, 0xbc9180902e30e93e), // {0x1.5dfffebfc3481p+0, -0x1.180902e30e93ep-54}
    ];
}

const OFF: u64 = 0x3fe6000000000000;
/// `asuint64(1.0 - 0x1.5b51p-5)`, `asuint64(1.0 + 0x1.6ab2p-5)`: the arguments next to 1.
const LO: u64 = 0x3feea4af00000000;
const HI: u64 = 0x3ff0b55900000000;

#[inline(always)]
fn f(bits: u64) -> f64 {
    f64::from_bits(bits)
}

/// `log2(x)` with glibc 2.35's bits (see the module's notes).
#[inline(always)]
pub fn log2_glibc(x: f64) -> f64 {
    use log2_data::*;
    let mut ix = x.to_bits();
    let top = (ix >> 48) as u32;
    let invln2hi = f(INVLN2HI);
    let invln2lo = f(INVLN2LO);
    if ix.wrapping_sub(LO) < HI - LO {
        // close to 1.0
        if ix == 1.0f64.to_bits() {
            return 0.0;
        }
        let b = |k: usize| f(POLY1[k]);
        let r = x - 1.0;
        let rhi = f(r.to_bits() & (u64::MAX << 32));
        let rlo = r - rhi;
        let hi = rhi * invln2hi;
        let mut lo = rlo * invln2hi + r * invln2lo;
        let r2 = r * r;
        let r4 = r2 * r2;
        let p = r2 * (b(0) + r * b(1));
        let y = hi + p;
        lo += hi - y + p;
        lo += r4
            * (b(2)
                + r * b(3)
                + r2 * (b(4) + r * b(5))
                + r4 * (b(6) + r * b(7) + r2 * (b(8) + r * b(9))));
        return y + lo;
    }
    if top.wrapping_sub(0x0010) >= 0x7ff0 - 0x0010 {
        // x < 0x1p-1022 or inf or nan
        if ix.wrapping_mul(2) == 0 {
            // __math_divzero(1)
            return f64::NEG_INFINITY;
        }
        if ix == f64::INFINITY.to_bits() {
            return x;
        }
        if (top & 0x8000) != 0 || (top & 0x7ff0) == 0x7ff0 {
            return invalid_f64(x);
        }
        // subnormal: normalize it
        ix = (x * f(0x4330000000000000)).to_bits(); // x * 0x1p52
        ix = ix.wrapping_sub(52u64 << 52);
    }
    let tmp = ix.wrapping_sub(OFF);
    let i = ((tmp >> (52 - 6)) % 64) as usize;
    let k = ((tmp as i64) >> 52) as i32;
    let iz = ix.wrapping_sub(tmp & (0xfffu64 << 52));
    let invc = f(TAB[i].0);
    let logc = f(TAB[i].1);
    let z = f(iz);
    let kd = f64::from(k);
    // r ~= z/c - 1, |r| < 1/(2*N), without a fused multiply-add
    let r = (z - f(TAB2[i].0) - f(TAB2[i].1)) * invc;
    let rhi = f(r.to_bits() & (u64::MAX << 32));
    let rlo = r - rhi;
    let t1 = rhi * invln2hi;
    let t2 = rlo * invln2hi + r * invln2lo;
    // hi + lo = r/ln2 + log2(c) + k
    let t3 = kd + logc;
    let hi = t3 + t1;
    let lo = t3 - hi + t1 + t2;
    // log2(r+1) = r/ln2 + r^2*poly(r)
    let a = |k: usize| f(POLY[k]);
    let r2 = r * r;
    let r4 = r2 * r2;
    let p = a(0) + r * a(1) + r2 * (a(2) + r * a(3)) + r4 * (a(4) + r * a(5));
    lo + r2 * p + hi
}

#[cfg(test)]
mod tests {
    use super::log2_glibc;

    #[test]
    fn special_values() {
        assert_eq!(log2_glibc(1.0).to_bits(), 0);
        assert_eq!(log2_glibc(2.0), 1.0);
        assert_eq!(log2_glibc(0.5), -1.0);
        assert_eq!(log2_glibc(0.0), f64::NEG_INFINITY);
        assert_eq!(log2_glibc(-0.0), f64::NEG_INFINITY);
        assert_eq!(log2_glibc(f64::INFINITY), f64::INFINITY);
        assert_eq!(log2_glibc(-1.0).to_bits(), 0xfff8_0000_0000_0000);
        assert_eq!(
            log2_glibc(f64::NEG_INFINITY).to_bits(),
            0xfff8_0000_0000_0000
        );
        assert_eq!(
            log2_glibc(f64::from_bits(0x7ff0_0000_0000_0001)).to_bits(),
            0x7ff8_0000_0000_0001
        );
        assert_eq!(log2_glibc(f64::from_bits(1)), -1074.0);
    }

    /// FNV-1a of the result bits on 65,536 seeded arguments (SplitMix64 from 0x123456789abcdef0:
    /// one third uniform on (0, 1000), one third random bit patterns, one third subnormal or
    /// zero), the arguments of `validation/probes/coremath-log2/`'s `seeded_hash`: the system's `log2` (glibc 2.35, Linux x86-64) gives it, NaN bits included (every platform must give it).
    #[test]
    fn seeded_hash() {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        let mut s = 0x1234_5678_9abc_def0u64;
        for i in 0..65_536u64 {
            s = s.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = s;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            let r = z ^ (z >> 31);
            let x = match i % 3 {
                0 => (r >> 11) as f64 / (1u64 << 53) as f64 * 1e3,
                1 => f64::from_bits(r),
                _ => f64::from_bits(r & 0x000f_ffff_ffff_ffff),
            };
            h = (h ^ log2_glibc(x).to_bits()).wrapping_mul(0x0100_0000_01b3);
        }
        assert_eq!(h, 0xd5a3_591b_5b6b_fea5);
    }
}
