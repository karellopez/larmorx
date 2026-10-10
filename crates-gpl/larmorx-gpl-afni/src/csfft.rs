// SPDX-License-Identifier: GPL-3.0-or-later
// Translated to Rust from AFNI 25.2.09 src/csfft.c.
// Major portions Copyright (C) 1994-2000 Medical College of Wisconsin (AFNI, Robert W. Cox et al.;
// the radix-2 code by Andrzej Jesmanowicz), released under the GNU GPL version 2 or any later
// version.
// Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the original.
//! AFNI's complex FFT `csfft_cox`, bit for bit.
//!
//! Translated from AFNI 25.2.09 `src/csfft.c` (original by Andrzej Jesmanowicz and RW Cox).
//! 3dTshift's Fourier interpolation (`fft_shift2` in `thd_shift2.c`) calls
//! `csfft_cox(-1, nup, row)` and `csfft_cox(+1, nup, row)` with
//! `nup = csfft_nextup_one35(ntt + 4)`; [`csfft_cox`] and [`CsfftPlan`] return the same `f32`
//! bits as AFNI for every length that `csfft_cox` computes with its own code.
//!
//! # Scope
//!
//! `csfft_cox(mode, n, x)` hands a length to AFNI's `fftn` package (not ported) unless
//! `csfft_nextup_even(n) == n` (with `internal_check = 1`) and `n <= 32768`, i.e. unless
//! `n = 2^a·3^b·5^c` with `a >= 1` and `b, c <= 3`. Every such length is ported:
//!
//! - powers of two: the `fft2`/`fft4` macros, the generated fully unrolled `fft8`, `fft16`,
//!   `fft32`, the decimations `fft64` (by 2), `fft128`/`fft256`/`fft512` and `fft_4dec`
//!   (by 4, used for 1024..=32768);
//! - factors of 3 and 5: the recursive decimations `fft_3dec` and `fft_5dec`;
//! - the general power-of-two loop of `csfft_cox` with the `csfft_trigconsts` tables (no
//!   AFNI-routed length reaches it, since the switch covers every power of two up to 32768;
//!   it is kept for completeness).
//!
//! [`csfft_nextup`], [`csfft_nextup_one35`] and [`csfft_nextup_even`] are ported too.
//! `csfft_many` and the `csfft_scale_inverse` switch are not: 3dTshift never sets
//! `sclinv`, so AFNI's default (no `1/n` scaling for `mode > 0`) is what is reproduced.
//!
//! # Differences from the C code (results unchanged)
//!
//! - No global state. The C routines cache their twiddle tables and work arrays in statics
//!   (`csplus`/`csminus`, per-routine and per-recursion-level `cs`/`aa`/`bb`/...); here a
//!   [`CsfftPlan`] computes the same tables once, with the same expressions, and is
//!   immutable (`Send + Sync`). Work arrays come from a caller-provided scratch slice.
//! - `fft8`, `fft16` and `fft32` are written as one loop instead of 1300 generated lines. The
//!   loop reproduces each generated butterfly exactly; checked by parsing every statement of
//!   the C routines: a bit-reversal by pairwise swaps (pure data moves), then the radix-2
//!   stages `m = 1, 2, ..., n/2`, where the butterfly `(i1, i1 + m)` with `i0 = i1 mod 2m`
//!   uses `csp[m - 1 + i0]` and is written in one of three ways (see `Butterflies`). The
//!   order of butterflies within a stage differs from the generated code; they touch disjoint
//!   elements, so the results do not depend on it.
//!
//! # Floating-point semantics
//!
//! AFNI is built for x86-64 with `gcc -O2` (SSE arithmetic, no `-ffast-math`, no FMA in the
//! object code), so every `float` operation rounds to `f32` and nothing is contracted, as in
//! Rust. C's implicit conversions are kept explicitly:
//!
//! - `fft_3dec`: `(bbr-ccr)*SS3` multiplies the `float` difference by the `double` constant
//!   `0.8660254038` in double precision and rounds the product to `f32` on assignment;
//!   rounding `SS3` to `f32` first would change results. `t4*CC3` is a double product too
//!   (exact, as `CC3 = -0.5`).
//! - `fft_5dec`: `s72 = ±SIN72` and `c72 = COS72` round the double constants to `f32`;
//!   `c2 = c72*c72 - s72*s72` is `f32` arithmetic, `s2 = 2.0*c72*s72` a double product
//!   rounded to `f32`; `cs[k].i * ss` converts the `int` `ss = ±1` to `f32`.
//! - Twiddle tables: `cos(k*th)` / `sin(k*th)` take a double argument (`k` is an `int`,
//!   `th = 2.0*PI/N` or `PI/32.0`) and round the double result to `f32`.
//!   `csfft_trigconsts` computes `al = f1*PI/f2` in double (`f1 = ±1.0f`, `f2 = m` as a
//!   `float`), rounds `cos(al)`/`sin(al)` to `f32` and then runs an `f32` recurrence.
//! - `mode > 0` selects the `exp(+i·…)` branch everywhere; any other value, including 0, the
//!   `exp(-i·…)` one.
//!
//! The double `cos`/`sin` come from glibc in AFNI (gcc merges each pair into one `sincos`
//! call) and from [`larmorx_core::math`] here (correctly rounded, the same bits on every
//! platform). Only their values rounded to `f32` matter: two double results within 1 ulp of
//! each other round to different `f32` values only when they straddle an `f32` rounding
//! boundary. Over the 539,282 arguments of all tables of all supported lengths, the correctly
//! rounded values and glibc 2.35's agree after rounding to `f32` (`tools/csfft-verify`).
//!
//! # Licence
//!
//! `csfft.c` is one of the exceptions to AFNI's public-domain notice (`LICENSE.txt`): its
//! header reads "Major portions of this software are copyrighted by the Medical College of
//! Wisconsin, 1994-2000, and are released under the Gnu General Public License, Version 2",
//! and `doc/README/README.copyright` allows "any later edition". The routines translated here
//! date from the MCW period (the radix-2 code is adapted from Jesmanowicz's; the unrolled
//! kernels are from November 1998, the radix-3/5 and decimation-by-4 code from August 1999).
//! This translation is therefore distributed under the GPL, version 3 or later, in the
//! `larmorx-gpl` package (docs/licensing.md).

use std::f64::consts::PI;
use std::fmt;

/// AFNI's `complex` (`mrilib.h`: `typedef struct complex { float r, i; } complex;`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Complex32 {
    /// Real part.
    pub r: f32,
    /// Imaginary part.
    pub i: f32,
}

impl Complex32 {
    /// The complex number with real part `r` and imaginary part `i`.
    pub const fn new(r: f32, i: f32) -> Self {
        Complex32 { r, i }
    }
}

/// The longest length `csfft_cox` computes with its own code.
pub const CSFFT_COX_MAX_LEN: usize = 32768;

/// A length that `csfft_cox` hands to AFNI's `fftn` package, which is not ported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnsupportedLength(pub usize);

impl fmt::Display for UnsupportedLength {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "csfft_cox: AFNI computes length {} with fftn, which is not ported; csfft_cox's own \
             code needs n = 2^a*3^b*5^c with a >= 1, b <= 3, c <= 3 and n <= {}",
            self.0, CSFFT_COX_MAX_LEN
        )
    }
}

impl std::error::Error for UnsupportedLength {}

/// `cos` of a double argument, correctly rounded (glibc's in AFNI; see the module docs).
fn cos(x: f64) -> f64 {
    larmorx_core::math::cos(x)
}

/// `sin` of a double argument, correctly rounded (glibc's in AFNI; see the module docs).
fn sin(x: f64) -> f64 {
    larmorx_core::math::sin(x)
}

/// In-place complex FFT of `x`, bit-identical to AFNI's `csfft_cox(mode, x.len(), x)`.
///
/// Computes `X[k] = Σ_j x[j]·exp(s·2πi·jk/n)` with `s = +1` if `mode > 0` and `s = -1`
/// otherwise, without scaling (AFNI's inverse, `mode = +1`, is not divided by `n`). Lengths 0
/// and 1 are left unchanged, as in AFNI.
///
/// This builds a [`CsfftPlan`] on every call; build one plan and reuse it when transforming
/// many arrays of the same length.
///
/// # Panics
///
/// If AFNI would compute this length with `fftn` instead (see [`csfft_cox_handles`]).
pub fn csfft_cox(mode: i32, x: &mut [Complex32]) {
    match CsfftPlan::new(x.len()) {
        Ok(plan) => plan.execute(mode, x),
        Err(e) => panic!("{e}"),
    }
}

/// Whether AFNI's `csfft_cox` computes length `n` with its own code (and so this port), rather
/// than with `fftn`: `n <= 1`, or `csfft_nextup_even(n) == n` and `n <= 32768`.
pub fn csfft_cox_handles(n: usize) -> bool {
    n <= 1 || (n <= CSFFT_COX_MAX_LEN && csfft_nextup_even(n) == n)
}

/// `csfft_cox` for one length, with AFNI's twiddle tables precomputed.
///
/// The plan is immutable and can be shared between threads.
pub struct CsfftPlan {
    n: usize,
    root: Node,
    scratch_len: usize,
}

impl CsfftPlan {
    /// Prepares the transform of length `n`.
    ///
    /// # Errors
    ///
    /// [`UnsupportedLength`] if AFNI would compute this length with `fftn`.
    pub fn new(n: usize) -> Result<Self, UnsupportedLength> {
        // csfft_cox: `if( idim <= 1 ) return ;` comes before the fftn routing.
        let root = if n <= 1 { Node::Identity } else { cox_node(n)? };
        let scratch_len = root.scratch_len(n);
        Ok(CsfftPlan {
            n,
            root,
            scratch_len,
        })
    }

    /// The transform length.
    pub fn size(&self) -> usize {
        self.n
    }

    /// The scratch length [`execute_with_scratch`](Self::execute_with_scratch) needs (< 2n).
    pub fn scratch_len(&self) -> usize {
        self.scratch_len
    }

    /// Transforms `x` in place (see [`csfft_cox`]), allocating the scratch space.
    ///
    /// # Panics
    ///
    /// If `x.len()` is not the plan's length.
    pub fn execute(&self, mode: i32, x: &mut [Complex32]) {
        let mut scratch = vec![Complex32::default(); self.scratch_len];
        self.execute_with_scratch(mode, x, &mut scratch);
    }

    /// Transforms `x` in place (see [`csfft_cox`]) using `scratch` as work space.
    ///
    /// # Panics
    ///
    /// If `x.len()` is not the plan's length, or `scratch` is shorter than
    /// [`scratch_len`](Self::scratch_len).
    pub fn execute_with_scratch(&self, mode: i32, x: &mut [Complex32], scratch: &mut [Complex32]) {
        assert_eq!(
            x.len(),
            self.n,
            "csfft_cox: the plan is for length {}",
            self.n
        );
        assert!(
            scratch.len() >= self.scratch_len,
            "csfft_cox: scratch has length {}, {} needed",
            scratch.len(),
            self.scratch_len
        );
        self.root.run(mode, x, scratch);
    }
}

impl fmt::Debug for CsfftPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CsfftPlan")
            .field("n", &self.n)
            .finish_non_exhaustive()
    }
}

/// One routine of `csfft.c` with its tables; the decimations hold the routine they recurse
/// into for each subarray (all subarrays of one call have the same length).
enum Node {
    /// Length 1: `case 1: break ;` in `fft_3dec`/`fft_5dec` (and `idim <= 1` in `csfft_cox`).
    Identity,
    /// The `fft2` macro.
    Fft2,
    /// The `fft4` macro (`USE_FFT4_MACRO` is defined, so the `fft4` routine is unused).
    Fft4,
    /// `fft8`/`fft16`/`fft32`, or the general power-of-two loop of `csfft_cox`, with the
    /// `csfft_trigconsts(n)` tables.
    Radix2 {
        trig: TrigConsts,
        butterflies: Butterflies,
    },
    /// `fft64`: decimation by 2 into two `fft32`.
    Fft64 { cs: Vec<Complex32>, sub: Box<Node> },
    /// `fft128`, `fft256`, `fft512` and `fft_4dec` (N = 1024..=32768): decimation by 4.
    Dec4 { cs: Vec<Complex32>, sub: Box<Node> },
    /// `fft_3dec`: decimation by 3.
    Dec3 { cs: Vec<Complex32>, sub: Box<Node> },
    /// `fft_5dec`: decimation by 5.
    Dec5 { cs: Vec<Complex32>, sub: Box<Node> },
}

impl Node {
    /// Scratch needed to transform `n` points: each decimation copies its input into `n`
    /// scratch elements (the C `aa`, `bb`, ... arrays) before recursing.
    fn scratch_len(&self, n: usize) -> usize {
        match self {
            Node::Identity | Node::Fft2 | Node::Fft4 | Node::Radix2 { .. } => 0,
            Node::Fft64 { sub, .. } => n + sub.scratch_len(n / 2),
            Node::Dec4 { sub, .. } => n + sub.scratch_len(n / 4),
            Node::Dec3 { sub, .. } => n + sub.scratch_len(n / 3),
            Node::Dec5 { sub, .. } => n + sub.scratch_len(n / 5),
        }
    }

    fn run(&self, mode: i32, x: &mut [Complex32], scratch: &mut [Complex32]) {
        match self {
            Node::Identity => {}
            Node::Fft2 => fft2(x),
            Node::Fft4 => fft4(mode, x),
            Node::Radix2 { trig, butterflies } => {
                bit_reverse(x);
                radix2_stages(x, trig.select(mode), *butterflies);
            }
            Node::Fft64 { cs, sub } => fft64(mode, x, cs, sub, scratch),
            Node::Dec4 { cs, sub } => fft_4dec(mode, x, cs, sub, scratch),
            Node::Dec3 { cs, sub } => fft_3dec(mode, x, cs, sub, scratch),
            Node::Dec5 { cs, sub } => fft_5dec(mode, x, cs, sub, scratch),
        }
    }
}

/// `csfft_cox(mode, n, x)` for `n >= 2`: the `fftn` routing test, then the dispatch.
fn cox_node(n: usize) -> Result<Node, UnsupportedLength> {
    if !csfft_cox_handles(n) {
        return Err(UnsupportedLength(n));
    }
    // switch( idim ): every power of two from 2 to 32768.
    if n.is_power_of_two() {
        return Ok(pow2_node(n));
    }
    let th = 2.0 * PI / n as f64;
    if n.is_multiple_of(3) {
        let m = n / 3;
        return Ok(Node::Dec3 {
            cs: twiddle_table(th, 2 * m),
            sub: Box::new(dec_sub(m)?),
        });
    }
    if n.is_multiple_of(5) {
        let m = n / 5;
        return Ok(Node::Dec5 {
            cs: twiddle_table(th, 4 * m),
            sub: Box::new(dec_sub(m)?),
        });
    }
    // The general power-of-two routine (unreachable for the lengths routed here).
    Ok(Node::Radix2 {
        trig: TrigConsts::new(n),
        butterflies: Butterflies::General,
    })
}

/// The power-of-two routines; `csfft_cox`'s `switch(idim)` (2..=32768), `fft_4dec`'s
/// `switch(N)` and the `switch(M)` of `fft_3dec`/`fft_5dec` (2..=2048) agree on the routine
/// for each length. `n` must be a power of two in 2..=32768.
fn pow2_node(n: usize) -> Node {
    match n {
        2 => Node::Fft2,
        4 => Node::Fft4,
        8 | 16 => Node::Radix2 {
            trig: TrigConsts::new(n),
            butterflies: Butterflies::UnitSkipped,
        },
        32 => Node::Radix2 {
            trig: TrigConsts::new(n),
            butterflies: Butterflies::UnitAndQuarterSkipped,
        },
        // fft64: `double th = (PI/32.0)`, 32 entries (cs[0] is never used).
        64 => Node::Fft64 {
            cs: twiddle_table(PI / 32.0, 32),
            sub: Box::new(pow2_node(32)),
        },
        // fft128/fft256/fft512 (sub = fft32/fft64/fft128) and fft_4dec (sub = fft256, fft512,
        // fft_4dec(1024), ...): `double th = (2.0*PI/N)`, M3 = 3N/4 entries.
        _ => Node::Dec4 {
            cs: twiddle_table(2.0 * PI / n as f64, 3 * (n / 4)),
            sub: Box::new(pow2_node(n / 4)),
        },
    }
}

/// The routine `fft_3dec`/`fft_5dec` use for their subarrays of length `m`: `switch(M)`, whose
/// `default` calls `csfft_cox` (routing test included).
fn dec_sub(m: usize) -> Result<Node, UnsupportedLength> {
    match m {
        1 => Ok(Node::Identity),
        2 | 4 | 8 | 16 | 32 | 64 | 128 | 256 | 512 | 1024 | 2048 => Ok(pow2_node(m)),
        _ => cox_node(m),
    }
}

/// `cs[0] = (1, 0)` and `cs[k] = ((float)cos(k*th), (float)sin(k*th))` for `k` in `1..len`: the
/// tables of `fft64`, `fft128`/`fft256`/`fft512`, `fft_4dec`, `fft_3dec` and `fft_5dec`.
fn twiddle_table(th: f64, len: usize) -> Vec<Complex32> {
    let mut cs = Vec::with_capacity(len);
    cs.push(Complex32::new(1.0, 0.0));
    for k in 1..len {
        let a = k as f64 * th;
        cs.push(Complex32::new(cos(a) as f32, sin(a) as f32));
    }
    cs
}

/// The `csplus` (`mode > 0`) and `csminus` tables of `csfft_trigconsts(idim)`.
struct TrigConsts {
    plus: Vec<Complex32>,
    minus: Vec<Complex32>,
}

impl TrigConsts {
    fn new(idim: usize) -> Self {
        TrigConsts {
            plus: trigconsts_table(idim, 1.0),
            minus: trigconsts_table(idim, -1.0),
        }
    }

    /// `csp = (mode > 0) ? csplus : csminus`.
    fn select(&self, mode: i32) -> &[Complex32] {
        if mode > 0 { &self.plus } else { &self.minus }
    }
}

/// One table of `csfft_trigconsts(n)`: for each stage `m = 1, 2, ..., n/2`, `cs[m-1] = (1, 0)`
/// followed by `m` powers of `exp(i·f1·π/m)` from an `f32` recurrence (`cs[m-1]` is
/// overwritten by the next stage).
fn trigconsts_table(n: usize, f1: f32) -> Vec<Complex32> {
    let mut cs = vec![Complex32::default(); n];
    let mut m = 1usize;
    let mut k = 0usize;
    while n > m {
        let i3 = m << 1;
        let f2 = m as f32;
        let al = f64::from(f1) * PI / f64::from(f2);
        let co = cos(al) as f32;
        let si = sin(al) as f32;
        cs[k] = Complex32::new(1.0, 0.0);
        for _ in 0..m {
            k += 1;
            let r0 = cs[k - 1];
            cs[k] = Complex32::new(r0.r * co - r0.i * si, r0.i * co + r0.r * si);
        }
        m = i3;
    }
    cs
}

/// How a radix-2 routine writes the butterfly that multiplies by `csp[k]`, `k = m - 1 + i0`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Butterflies {
    /// `csfft_cox`'s general loop: always `f1 = x.r*co - x.i*si ; f3 = x.r*si + x.i*co`, also
    /// for `i0 = 0` where `csp[k] = (1, 0)`.
    General,
    /// `fft8`, `fft16` (generated by `fftprint.c`): for `i0 = 0` ("cos=1 sin=0") no multiply,
    /// `f1 = x.r ; f3 = x.i`; otherwise as `General`.
    UnitSkipped,
    /// `fft32`: as `UnitSkipped`, and for `i0 = m/2` ("cos=0 twiddles")
    /// `f1 = - x.i * csp[k].i ; f3 = x.r * csp[k].i`.
    UnitAndQuarterSkipped,
}

/// The data swapping of the radix-2 routines: bit-reversal permutation.
fn bit_reverse(x: &mut [Complex32]) {
    let n = x.len();
    let i2 = n >> 1;
    let mut i1 = 0usize;
    for i0 in 0..n {
        if i1 > i0 {
            x.swap(i0, i1);
        }
        let mut m = i2;
        while m != 0 && i1 >= m {
            i1 -= m;
            m >>= 1;
        }
        i1 += m;
    }
}

/// The butterflies of the radix-2 routines (decimation in time, after [`bit_reverse`]).
fn radix2_stages(x: &mut [Complex32], csp: &[Complex32], butterflies: Butterflies) {
    let n = x.len();
    let mut m = 1usize;
    let mut k = 0usize;
    while n > m {
        let i3 = m << 1;
        for i0 in 0..m {
            let c = csp[k];
            let mut i1 = i0;
            while i1 < n {
                let r1 = x[i1 + m];
                let (f1, f3) = if i0 == 0 && butterflies != Butterflies::General {
                    (r1.r, r1.i)
                } else if i0 == m / 2 && butterflies == Butterflies::UnitAndQuarterSkipped {
                    (-r1.i * c.i, r1.r * c.i)
                } else {
                    (r1.r * c.r - r1.i * c.i, r1.r * c.i + r1.i * c.r)
                };
                let r0 = x[i1];
                x[i1 + m] = Complex32::new(r0.r - f1, r0.i - f3);
                x[i1] = Complex32::new(r0.r + f1, r0.i + f3);
                i1 += i3;
            }
            k += 1;
        }
        m = i3;
    }
}

/// The `fft2` macro.
fn fft2(x: &mut [Complex32]) {
    let (x0, x1) = (x[0], x[1]);
    x[0] = Complex32::new(x0.r + x1.r, x0.i + x1.i);
    x[1] = Complex32::new(x0.r - x1.r, x0.i - x1.i);
}

/// The `fft4` macro.
fn fft4(mode: i32, x: &mut [Complex32]) {
    let acpr = x[0].r + x[2].r;
    let acmr = x[0].r - x[2].r;
    let bdpr = x[1].r + x[3].r;
    let bdmr = x[1].r - x[3].r;
    let acpi = x[0].i + x[2].i;
    let acmi = x[0].i - x[2].i;
    let bdpi = x[1].i + x[3].i;
    let bdmi = x[1].i - x[3].i;
    x[0] = Complex32::new(acpr + bdpr, acpi + bdpi);
    x[2] = Complex32::new(acpr - bdpr, acpi - bdpi);
    if mode > 0 {
        x[1] = Complex32::new(acmr - bdmi, acmi + bdmr);
        x[3] = Complex32::new(acmr + bdmi, acmi - bdmr);
    } else {
        x[1] = Complex32::new(acmr + bdmi, acmi - bdmr);
        x[3] = Complex32::new(acmr - bdmi, acmi + bdmr);
    }
}

/// `b·exp(±iθ)` for the table entry `t = (cos θ, sin θ)`, written as `fft64`, `fft_4dec` and
/// `fft_3dec` write it (`t1 = b.r`, `t2 = b.i`, `tr = t.r`, `ti = t.i`):
/// `(tr*t1 - ti*t2, tr*t2 + ti*t1)` for `mode > 0`, `(tr*t1 + ti*t2, tr*t2 - ti*t1)` otherwise.
#[inline]
fn rotate(plus: bool, t: Complex32, b: Complex32) -> (f32, f32) {
    let (t1, t2, tr, ti) = (b.r, b.i, t.r, t.i);
    if plus {
        (tr * t1 - ti * t2, tr * t2 + ti * t1)
    } else {
        (tr * t1 + ti * t2, tr * t2 - ti * t1)
    }
}

/// The "load subarrays, and FFT each one" step of the decimations: `x[R*k + j]` goes to
/// element `k` of subarray `j` (the C arrays `aa`, `bb`, ...), stored one after another in
/// `scratch[..n]`; each subarray is then transformed by `sub`. Returns the subarrays.
fn decimate<'a>(
    mode: i32,
    x: &[Complex32],
    radix: usize,
    sub: &Node,
    scratch: &'a mut [Complex32],
) -> &'a [Complex32] {
    let n = x.len();
    let m = n / radix;
    let (buf, rest) = scratch.split_at_mut(n);
    for (k, group) in x.chunks_exact(radix).enumerate() {
        for (j, &v) in group.iter().enumerate() {
            buf[j * m + k] = v;
        }
    }
    for part in buf.chunks_exact_mut(m) {
        sub.run(mode, part, rest);
    }
    buf
}

/// `fft64`: two `fft32` and a radix-2 recombination; `k = 0` uses no twiddle.
fn fft64(mode: i32, x: &mut [Complex32], cs: &[Complex32], sub: &Node, scratch: &mut [Complex32]) {
    let buf = decimate(mode, x, 2, sub, scratch);
    let (aa, bb) = buf.split_at(32);

    x[0] = Complex32::new(aa[0].r + bb[0].r, aa[0].i + bb[0].i);
    x[32] = Complex32::new(aa[0].r - bb[0].r, aa[0].i - bb[0].i);

    let plus = mode > 0;
    for k in 1..32 {
        let (t1, t2) = rotate(plus, cs[k], bb[k]);
        let (akr, aki) = (aa[k].r, aa[k].i);
        x[k] = Complex32::new(akr + t1, aki + t2);
        x[k + 32] = Complex32::new(akr - t1, aki - t2);
    }
}

/// `fft_4dec` (and the identical `fft128`, `fft256`, `fft512`): four sub-FFTs and a radix-4
/// recombination.
fn fft_4dec(
    mode: i32,
    x: &mut [Complex32],
    cs: &[Complex32],
    sub: &Node,
    scratch: &mut [Complex32],
) {
    let m = x.len() / 4;
    let (m2, m3) = (2 * m, 3 * m);
    let buf = decimate(mode, x, 4, sub, scratch);
    let (aa, rest) = buf.split_at(m);
    let (bb, rest) = rest.split_at(m);
    let (cc, dd) = rest.split_at(m);

    let plus = mode > 0;
    for k in 0..m {
        let (bbr, bbi) = rotate(plus, cs[k], bb[k]); // b[k]*exp(±i*2*Pi*k/N)
        let (ccr, cci) = rotate(plus, cs[2 * k], cc[k]); // c[k]*exp(±i*4*Pi*k/N)
        let (ddr, ddi) = rotate(plus, cs[3 * k], dd[k]); // d[k]*exp(±i*6*Pi*k/N)
        let (aar, aai) = (aa[k].r, aa[k].i);

        let acpr = aar + ccr;
        let acmr = aar - ccr;
        let bdpr = bbr + ddr;
        let bdmr = bbr - ddr;
        let acpi = aai + cci;
        let acmi = aai - cci;
        let bdpi = bbi + ddi;
        let bdmi = bbi - ddi;

        x[k] = Complex32::new(acpr + bdpr, acpi + bdpi);
        x[k + m2] = Complex32::new(acpr - bdpr, acpi - bdpi);
        if plus {
            x[k + m] = Complex32::new(acmr - bdmi, acmi + bdmr);
            x[k + m3] = Complex32::new(acmr + bdmi, acmi - bdmr);
        } else {
            x[k + m] = Complex32::new(acmr + bdmi, acmi - bdmr);
            x[k + m3] = Complex32::new(acmr - bdmi, acmi + bdmr);
        }
    }
}

/// `CC3`: cos(2π/3), a `double` constant.
const CC3: f64 = -0.5;
/// `SS3`: sin(2π/3) to 10 digits, a `double` constant (products with it are double).
const SS3: f64 = 0.8660254038;

/// `fft_3dec`: three sub-FFTs and a radix-3 recombination.
fn fft_3dec(
    mode: i32,
    x: &mut [Complex32],
    cs: &[Complex32],
    sub: &Node,
    scratch: &mut [Complex32],
) {
    let m = x.len() / 3;
    let m2 = 2 * m;
    let buf = decimate(mode, x, 3, sub, scratch);
    let (aa, rest) = buf.split_at(m);
    let (bb, cc) = rest.split_at(m);

    let plus = mode > 0;
    for k in 0..m {
        let (bbr, bbi) = rotate(plus, cs[k], bb[k]); // b[k]*exp(±i*2*Pi*k/N)
        let (ccr, cci) = rotate(plus, cs[2 * k], cc[k]); // c[k]*exp(±i*4*Pi*k/N)

        // float*double products are double, rounded to float on assignment.
        let t4 = bbr + ccr;
        let t1 = (f64::from(t4) * CC3) as f32;
        let t8 = bbi + cci;
        let t6 = (f64::from(t8) * CC3) as f32;
        let t5 = (f64::from(bbr - ccr) * SS3) as f32;
        let t2 = (f64::from(bbi - cci) * SS3) as f32;

        let (aar, aai) = (aa[k].r, aa[k].i);

        x[k] = Complex32::new(aar + t4, aai + t8);
        if plus {
            x[k + m] = Complex32::new((aar + t1) - t2, (aai + t6) + t5);
            x[k + m2] = Complex32::new((aar + t1) + t2, (aai + t6) - t5);
        } else {
            x[k + m] = Complex32::new((aar + t1) + t2, (aai + t6) - t5);
            x[k + m2] = Complex32::new((aar + t1) - t2, (aai + t6) + t5);
        }
    }
}

/// `COS72`: cos(72°) to 8 digits, a `double` constant.
const COS72: f64 = 0.30901699;
/// `SIN72`: sin(72°) to 8 digits, a `double` constant.
const SIN72: f64 = 0.95105652;

/// `b·exp(ss·iθ)` as `fft_5dec` writes it: `aj = cs.r ; bj = cs.i * ss ;`
/// `(aj*ak - bj*bk, aj*bk + bj*ak)` with `ak = b.r`, `bk = b.i`.
#[inline]
fn rotate5(t: Complex32, ss: f32, b: Complex32) -> (f32, f32) {
    let (ak, bk) = (b.r, b.i);
    let (aj, bj) = (t.r, t.i * ss);
    (aj * ak - bj * bk, aj * bk + bj * ak)
}

/// `fft_5dec`: five sub-FFTs and a radix-5 recombination.
fn fft_5dec(
    mode: i32,
    x: &mut [Complex32],
    cs: &[Complex32],
    sub: &Node,
    scratch: &mut [Complex32],
) {
    let m = x.len() / 5;
    let (m2, m3, m4) = (2 * m, 3 * m, 4 * m);
    let buf = decimate(mode, x, 5, sub, scratch);
    let (aa, rest) = buf.split_at(m);
    let (bb, rest) = rest.split_at(m);
    let (cc, rest) = rest.split_at(m);
    let (dd, ee) = rest.split_at(m);

    // `float c72, s72, c2, s2 ; int ss ;`
    let (s72, ss): (f32, i32) = if mode > 0 {
        (SIN72 as f32, 1)
    } else {
        ((-SIN72) as f32, -1)
    };
    let c72 = COS72 as f32;
    let c2 = c72 * c72 - s72 * s72;
    let s2 = (2.0 * f64::from(c72) * f64::from(s72)) as f32;
    let ss = ss as f32;

    for k in 0..m {
        let (bbr, bbi) = rotate5(cs[k], ss, bb[k]); // b[k]*exp(ss*2*Pi*k/N)
        let (ccr, cci) = rotate5(cs[2 * k], ss, cc[k]); // c[k]*exp(ss*4*Pi*k/N)
        let (ddr, ddi) = rotate5(cs[3 * k], ss, dd[k]); // d[k]*exp(ss*6*Pi*k/N)
        let (eer, eei) = rotate5(cs[4 * k], ss, ee[k]); // e[k]*exp(ss*8*Pi*k/N)
        let (aar, aai) = (aa[k].r, aa[k].i);

        let akp = bbr + eer;
        let akm = bbr - eer;
        let bkp = bbi + eei;
        let bkm = bbi - eei;
        let ajp = ccr + ddr;
        let ajm = ccr - ddr;
        let bjp = cci + ddi;
        let bjm = cci - ddi;

        x[k] = Complex32::new(aar + akp + ajp, aai + bkp + bjp);

        let ak = akp * c72 + ajp * c2 + aar;
        let bk = bkp * c72 + bjp * c2 + aai;
        let aj = akm * s72 + ajm * s2;
        let bj = bkm * s72 + bjm * s2;
        x[k + m] = Complex32::new(ak - bj, bk + aj);
        x[k + m4] = Complex32::new(ak + bj, bk - aj);

        let ak = akp * c2 + ajp * c72 + aar;
        let bk = bkp * c2 + bjp * c72 + aai;
        let aj = akm * s2 - ajm * s72;
        let bj = bkm * s2 - bjm * s72;
        x[k + m2] = Complex32::new(ak - bj, bk + aj);
        x[k + m3] = Complex32::new(ak + bj, bk - aj);
    }
}

/// `RMAX`: the largest power of 3 and of 5 in a length (and the recursion limit of the
/// decimations, never reached by the lengths routed here).
const RMAX: usize = 3;
/// `N35`: the number of products `3^p·5^q`, `p, q <= RMAX`.
const N35: usize = (RMAX + 1) * (RMAX + 1);

/// The tables of `csfft_nextup`: `tf = 3^p·5^q`, `dn` the power of two just below `tf`
/// (1 for `tf = 1`), sorted by the `f32` ratio `tf/dn` with the C bubble sort.
fn nextup_tables() -> ([usize; N35], [usize; N35]) {
    let mut tf = [0usize; N35];
    let mut dn = [0usize; N35];
    let mut rat = [0f32; N35];
    let mut i = 0;
    let mut tt = 1;
    for _p in 0..=RMAX {
        let mut ff = 1;
        for _q in 0..=RMAX {
            tf[i] = tt * ff;
            let mut j = 2;
            while j < tf[i] {
                j *= 2;
            }
            dn[i] = j / 2;
            rat[i] = tf[i] as f32 / dn[i] as f32;
            ff *= 5;
            i += 1;
        }
        tt *= 3;
    }
    loop {
        let mut swaps = 0;
        for i in 1..N35 {
            if rat[i - 1] > rat[i] {
                rat.swap(i - 1, i);
                tf.swap(i - 1, i);
                dn.swap(i - 1, i);
                swaps += 1;
            }
        }
        if swaps == 0 {
            return (tf, dn);
        }
    }
}

/// AFNI's `csfft_nextup`: the smallest `2^a·3^b·5^c >= idim` with `b, c <= 3` (1 for
/// `idim <= 1`).
///
/// The C function computes in `int` and overflows for `idim` above about 600000; this one
/// computes in `usize` and agrees with it wherever the C code is defined.
pub fn csfft_nextup(idim: usize) -> usize {
    let (tf, dn) = nextup_tables();
    let mut ibase = 1usize;
    loop {
        if idim <= ibase {
            return ibase;
        }
        for (&t, &d) in tf.iter().zip(&dn) {
            if d <= ibase && idim <= t * ibase / d {
                return t * ibase / d;
            }
        }
        ibase *= 2;
    }
}

/// AFNI's `csfft_nextup_one35`: the smallest even [`csfft_nextup`] value `>= idim` that is
/// divisible by neither 9 nor 25 (3dTshift's FFT length is `csfft_nextup_one35(ntt + 4)`).
pub fn csfft_nextup_one35(idim: usize) -> usize {
    let mut jj = idim;
    loop {
        jj = csfft_nextup(jj);
        if jj.is_multiple_of(9) || jj.is_multiple_of(25) || jj % 2 == 1 {
            jj += 1;
        } else {
            return jj;
        }
    }
}

/// AFNI's `csfft_nextup_even` as `csfft_cox` calls it (`internal_check = 1`): the smallest
/// even [`csfft_nextup`] value `>= idim`.
///
/// Called from outside `csfft.c` (`internal_check = 0`), AFNI's function only rounds `idim`
/// up to an even number.
pub fn csfft_nextup_even(idim: usize) -> usize {
    let mut jj = idim;
    loop {
        jj = csfft_nextup(jj);
        if jj % 2 == 1 {
            jj += 1;
        } else {
            return jj;
        }
    }
}

/// Access to private routines for the bit-exactness harness (`tools/csfft-verify`), which
/// compares them with AFNI's compiled code. Not part of the API.
#[cfg(feature = "verification")]
#[doc(hidden)]
pub mod verification {
    use super::*;

    /// `csfft_cox`'s general power-of-two loop on its own (no AFNI length reaches it).
    pub fn radix2_general(mode: i32, x: &mut [Complex32]) {
        let trig = TrigConsts::new(x.len());
        bit_reverse(x);
        radix2_stages(x, trig.select(mode), Butterflies::General);
    }

    /// The double `cos` the twiddle tables use.
    pub fn table_cos(a: f64) -> f64 {
        cos(a)
    }

    /// The double `sin` the twiddle tables use.
    pub fn table_sin(a: f64) -> f64 {
        sin(a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every length `csfft_cox` handles up to 32768 (`2^a·3^b·5^c`, `a >= 1`, `b, c <= 3`).
    fn cox_lengths() -> Vec<usize> {
        (2..=CSFFT_COX_MAX_LEN)
            .filter(|&n| csfft_cox_handles(n))
            .collect()
    }

    fn lcg(seed: &mut u64) -> f32 {
        *seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((*seed >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
    }

    #[test]
    fn pi_is_afnis_literal() {
        assert_eq!("3.141592653589793238462643".parse::<f64>().unwrap(), PI);
    }

    #[test]
    fn nextup_values() {
        // Values from AFNI 25.2.09's compiled csfft_nextup / csfft_nextup_one35.
        let nextup = [
            (0, 1),
            (1, 1),
            (2, 2),
            (3, 3),
            (7, 8),
            (11, 12),
            (13, 15),
            (17, 18),
        ];
        for (n, want) in nextup {
            assert_eq!(csfft_nextup(n), want, "csfft_nextup({n})");
        }
        let more = [
            (31, 32),
            (33, 36),
            (97, 100),
            (101, 108),
            (1000, 1000),
            (1025, 1080),
        ];
        for (n, want) in more {
            assert_eq!(csfft_nextup(n), want, "csfft_nextup({n})");
        }
        let one35 = [
            (3, 4),
            (7, 8),
            (13, 16),
            (17, 20),
            (100, 120),
            (154, 160),
            (204, 240),
        ];
        for (n, want) in one35 {
            assert_eq!(csfft_nextup_one35(n), want, "csfft_nextup_one35({n})");
        }
        assert_eq!(csfft_nextup_even(3), 4);
        assert_eq!(csfft_nextup_even(13), 16);
        assert_eq!(csfft_nextup_even(90), 90);
    }

    #[test]
    fn supported_lengths() {
        let lengths = cox_lengths();
        for &n in &lengths {
            let f = |p: usize| {
                let mut v = n;
                let mut e = 0;
                while v.is_multiple_of(p) {
                    v /= p;
                    e += 1;
                }
                e
            };
            let (a, b, c) = (f(2), f(3), f(5));
            assert!(a >= 1 && b <= 3 && c <= 3, "{n}");
            assert_eq!(n, (1 << a) * 3usize.pow(b) * 5usize.pow(c), "{n}");
            let plan = CsfftPlan::new(n).unwrap();
            assert!(plan.scratch_len() < 2 * n, "{n}");
        }
        assert_eq!(lengths.len(), 139);
        for n in [3, 5, 7, 9 * 9 * 2, 625 * 2, 65536] {
            assert_eq!(CsfftPlan::new(n).unwrap_err(), UnsupportedLength(n));
        }
        // 3dTshift's lengths are all handled.
        for ntt in 3..=4000 {
            assert!(csfft_cox_handles(csfft_nextup_one35(ntt + 4)), "{ntt}");
        }
    }

    #[test]
    fn plan_is_send_and_sync() {
        fn check<T: Send + Sync>() {}
        check::<CsfftPlan>();
    }

    #[test]
    #[should_panic(expected = "fftn")]
    fn unsupported_length_panics() {
        csfft_cox(-1, &mut [Complex32::default(); 7]);
    }

    #[test]
    fn matches_naive_dft() {
        let mut seed = 1;
        for n in [
            2, 4, 6, 8, 10, 12, 16, 20, 30, 32, 40, 48, 60, 64, 90, 120, 128, 250, 256,
        ] {
            let x: Vec<Complex32> = (0..n)
                .map(|_| Complex32::new(lcg(&mut seed), lcg(&mut seed)))
                .collect();
            for mode in [-1, 1] {
                let mut y = x.clone();
                csfft_cox(mode, &mut y);
                for (k, yk) in y.iter().enumerate() {
                    let (mut re, mut im) = (0f64, 0f64);
                    for (j, xj) in x.iter().enumerate() {
                        let a =
                            f64::from(mode.signum()) * 2.0 * PI * ((j * k) % n) as f64 / n as f64;
                        let (s, c) = a.sin_cos();
                        re += f64::from(xj.r) * c - f64::from(xj.i) * s;
                        im += f64::from(xj.r) * s + f64::from(xj.i) * c;
                    }
                    let err = (f64::from(yk.r) - re).hypot(f64::from(yk.i) - im);
                    assert!(err < 1e-5 * n as f64, "n={n} mode={mode} k={k} err={err}");
                }
            }
        }
    }

    #[test]
    fn inverse_of_forward_is_n_times_identity() {
        let mut seed = 7;
        for n in cox_lengths() {
            let x: Vec<Complex32> = (0..n)
                .map(|_| Complex32::new(lcg(&mut seed), lcg(&mut seed)))
                .collect();
            let plan = CsfftPlan::new(n).unwrap();
            let mut y = x.clone();
            plan.execute(-1, &mut y);
            plan.execute(1, &mut y);
            let scale = 1.0 / n as f64;
            let worst = x
                .iter()
                .zip(&y)
                .map(|(a, b)| {
                    (f64::from(b.r) * scale - f64::from(a.r))
                        .hypot(f64::from(b.i) * scale - f64::from(a.i))
                })
                .fold(0.0, f64::max);
            assert!(worst < 2e-5, "n={n} worst={worst}");
        }
    }

    #[test]
    fn scratch_is_reusable() {
        let plan = CsfftPlan::new(240).unwrap();
        let mut scratch = vec![Complex32::default(); plan.scratch_len()];
        let mut seed = 3;
        let x: Vec<Complex32> = (0..240)
            .map(|_| Complex32::new(lcg(&mut seed), 0.0))
            .collect();
        let mut a = x.clone();
        plan.execute(-1, &mut a);
        for _ in 0..2 {
            let mut b = x.clone();
            plan.execute_with_scratch(-1, &mut b, &mut scratch);
            assert!(
                a.iter()
                    .zip(&b)
                    .all(|(p, q)| p.r.to_bits() == q.r.to_bits() && p.i.to_bits() == q.i.to_bits())
            );
        }
    }
}
