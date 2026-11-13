// SPDX-License-Identifier: Apache-2.0
//! The proof that `larmorx_core::math`'s functions with glibc's bits (`sinf_glibc`, `cosf_glibc`,
//! `sincosf_glibc`, `logf_glibc`, `expf_glibc`, `powf_glibc`, `atanf_glibc`, `atan2f_glibc`,
//! `exp_glibc`, `log_glibc`, `pow_glibc`) give glibc 2.35's results, bit for bit (NaN results
//! included), and that the AFNI replicas' earlier translation of `sinf` and `cosf` gives the same.
//!
//! The system's functions are called as a black box from the C library's `libm.so.6`, opened
//! with `dlopen` (the current symbol versions: `logf@@GLIBC_2.27`, `exp@@GLIBC_2.29`, ...). On
//! x86-64 with FMA, glibc runs its FMA builds (`__sinf_fma`, ...), the oracles' builds.
//!
//! - `verify [--threads N] [--out result.json] [float] [pairs] [double]`: every float input of
//!   the one-argument float functions; the sampled families of `samples.rs` (more than 10^9
//!   arguments per function) and every pair of special values for the others.
//! - `hashes`: the hashes the unit tests in `larmorx_core::math` check, from the system's results
//!   (every 4099th float input; 65,536 seeded arguments of the double and two-argument functions).
//!
//! Exits 0 when every result is identical.

mod samples;

use std::ffi::{CStr, c_char, c_int, c_void};
use std::fmt::Write as _;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use larmorx_afni_replica::sincosf as afni;
use larmorx_core::math as lx;
use samples::{Family, random, splitmix64};

unsafe extern "C" {
    fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlerror() -> *const c_char;
    fn gnu_get_libc_version() -> *const c_char;
}

const RTLD_NOW: c_int = 2;

type F32 = extern "C" fn(f32) -> f32;
type F32x2 = extern "C" fn(f32, f32) -> f32;
type F64 = extern "C" fn(f64) -> f64;
type F64x2 = extern "C" fn(f64, f64) -> f64;
type SinCosF = extern "C" fn(f32, *mut f32, *mut f32);

/// The C library's functions, from `libm.so.6`.
struct Libm {
    sinf: F32,
    cosf: F32,
    sincosf: SinCosF,
    logf: F32,
    expf: F32,
    atanf: F32,
    tanhf: F32,
    expm1f: F32,
    acosf: F32,
    powf: F32x2,
    atan2f: F32x2,
    exp: F64,
    log: F64,
    pow: F64x2,
}

fn symbol(h: *mut c_void, name: &CStr) -> *mut c_void {
    // SAFETY: a plain call of the dynamic loader with a NUL-terminated name.
    let f = unsafe { dlsym(h, name.as_ptr()) };
    if f.is_null() {
        // SAFETY: dlerror returns a static NUL-terminated string after a failed call.
        panic!("dlsym({name:?}): {:?}", unsafe {
            CStr::from_ptr(dlerror())
        });
    }
    f
}

impl Libm {
    fn open() -> Libm {
        // SAFETY: plain calls of the C library's dynamic loader; every symbol is cast to the C
        // signature it has (`float sinf(float)`, `void sincosf(float, float *, float *)`, ...).
        unsafe {
            let h = dlopen(c"libm.so.6".as_ptr(), RTLD_NOW);
            if h.is_null() {
                panic!("dlopen(libm.so.6): {:?}", CStr::from_ptr(dlerror()));
            }
            use std::mem::transmute as t;
            Libm {
                sinf: t::<*mut c_void, F32>(symbol(h, c"sinf")),
                cosf: t::<*mut c_void, F32>(symbol(h, c"cosf")),
                sincosf: t::<*mut c_void, SinCosF>(symbol(h, c"sincosf")),
                logf: t::<*mut c_void, F32>(symbol(h, c"logf")),
                expf: t::<*mut c_void, F32>(symbol(h, c"expf")),
                atanf: t::<*mut c_void, F32>(symbol(h, c"atanf")),
                tanhf: t::<*mut c_void, F32>(symbol(h, c"tanhf")),
                expm1f: t::<*mut c_void, F32>(symbol(h, c"expm1f")),
                acosf: t::<*mut c_void, F32>(symbol(h, c"acosf")),
                powf: t::<*mut c_void, F32x2>(symbol(h, c"powf")),
                atan2f: t::<*mut c_void, F32x2>(symbol(h, c"atan2f")),
                exp: t::<*mut c_void, F64>(symbol(h, c"exp")),
                log: t::<*mut c_void, F64>(symbol(h, c"log")),
                pow: t::<*mut c_void, F64x2>(symbol(h, c"pow")),
            }
        }
    }

    fn sincos(&self, x: f32) -> (f32, f32) {
        let (mut s, mut c) = (0.0f32, 0.0f32);
        (self.sincosf)(x, &mut s, &mut c);
        (s, c)
    }
}

fn libc_version() -> String {
    // SAFETY: glibc's function returns a static NUL-terminated string.
    unsafe { CStr::from_ptr(gnu_get_libc_version()) }
        .to_string_lossy()
        .into_owned()
}

/// The outcome of one comparison: how many arguments, how many results identical bit for bit,
/// how many results were NaN (the reference's) and how many of those identical, the first
/// mismatches.
#[derive(Default)]
struct Tally {
    n: u64,
    same: u64,
    nan: u64,
    nan_same: u64,
    first: Vec<String>,
}

impl Tally {
    fn add(&mut self, o: Tally) {
        self.n += o.n;
        self.same += o.same;
        self.nan += o.nan;
        self.nan_same += o.nan_same;
        self.first.extend(o.first);
        self.first.sort();
        self.first.truncate(8);
    }

    #[inline]
    fn count(
        &mut self,
        reference_bits: u64,
        reference_nan: bool,
        ours_bits: u64,
        what: impl FnOnce() -> String,
    ) {
        self.n += 1;
        let ok = reference_bits == ours_bits;
        self.same += u64::from(ok);
        if reference_nan {
            self.nan += 1;
            self.nan_same += u64::from(ok);
        }
        if !ok && self.first.len() < 8 {
            self.first.push(what());
        }
    }
}

/// One line of the result: a comparison and its tally.
struct Check {
    function: &'static str,
    reference: &'static str,
    inputs: String,
    tally: Tally,
}

fn json_checks(checks: &[Check]) -> String {
    let mut s = String::new();
    for (k, c) in checks.iter().enumerate() {
        let t = &c.tally;
        let _ = writeln!(
            s,
            "    {{\"function\": \"{}\", \"reference\": \"{}\", \"inputs\": \"{}\", \"n\": {}, \
             \"identical\": {}, \"differing\": {}, \"nan_results\": {}, \"nan_identical\": {}, \
             \"first_mismatches\": [{}]}}{}",
            c.function,
            c.reference,
            c.inputs,
            t.n,
            t.same,
            t.n - t.same,
            t.nan,
            t.nan_same,
            t.first
                .iter()
                .map(|m| format!("\"{m}\""))
                .collect::<Vec<_>>()
                .join(", "),
            if k + 1 < checks.len() { "," } else { "" }
        );
    }
    s
}

/// Runs `work(chunk)` for chunks `0..chunks` on `threads` threads and adds up the tallies (one
/// tally per comparison, `k` of them).
fn parallel(
    threads: usize,
    chunks: u64,
    k: usize,
    work: impl Fn(u64) -> Vec<Tally> + Sync,
) -> Vec<Tally> {
    let next = AtomicU64::new(0);
    let total: Mutex<Vec<Tally>> = Mutex::new((0..k).map(|_| Tally::default()).collect());
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let c = next.fetch_add(1, Ordering::Relaxed);
                    if c >= chunks {
                        break;
                    }
                    let part = work(c);
                    let mut all = total.lock().unwrap();
                    for (a, p) in all.iter_mut().zip(part) {
                        a.add(p);
                    }
                }
            });
        }
    });
    total.into_inner().unwrap()
}

const CHUNK: u64 = 1 << 16;

#[inline]
fn b32(x: f32) -> u64 {
    u64::from(x.to_bits())
}

/// `sse2`: run with glibc's FMA builds switched off (`GLIBC_TUNABLES=glibc.cpu.hwcaps=-AVX2,-FMA`,
/// so that glibc's resolver picks `__sinf_sse2`, `__cosf_sse2`, `__sincosf_sse2`), the SSE2 builds
/// against the system's, every float input; and the FMA builds against them, which must differ
/// exactly where the two builds differ (34 inputs).
fn sse2(m: &Libm, threads: usize, out: Option<&str>) {
    let tunables = std::env::var("GLIBC_TUNABLES").unwrap_or_default();
    let names: [(&'static str, &'static str); 4] = [
        ("sinf_glibc_sse2", "glibc sinf (SSE2 build)"),
        ("cosf_glibc_sse2", "glibc cosf (SSE2 build)"),
        (
            "sincosf_glibc_sse2 (both outputs)",
            "glibc sincosf (SSE2 build)",
        ),
        (
            "sinf_glibc and cosf_glibc (FMA build)",
            "glibc sinf and cosf (SSE2 build): the builds' differences",
        ),
    ];
    let start = Instant::now();
    let t = parallel(threads, (1u64 << 32) / CHUNK, names.len(), |c| {
        let mut v: Vec<Tally> = (0..names.len()).map(|_| Tally::default()).collect();
        for i in c * CHUNK..(c + 1) * CHUNK {
            let x = f32::from_bits(i as u32);
            let what = || format!("{i:#010x}");
            let (gs, gc) = m.sincos(x);
            let (s, c2) = ((m.sinf)(x), (m.cosf)(x));
            v[0].count(b32(s), s.is_nan(), b32(lx::sinf_glibc_sse2(x)), what);
            v[1].count(b32(c2), c2.is_nan(), b32(lx::cosf_glibc_sse2(x)), what);
            let (os, oc) = lx::sincosf_glibc_sse2(x);
            v[2].count(
                b32(gs) << 32 | b32(gc),
                gs.is_nan(),
                b32(os) << 32 | b32(oc),
                what,
            );
            v[3].count(
                b32(s) << 32 | b32(c2),
                false,
                b32(lx::sinf_glibc(x)) << 32 | b32(lx::cosf_glibc(x)),
                what,
            );
        }
        v
    });
    let checks: Vec<Check> = names
        .into_iter()
        .zip(t)
        .map(|((function, reference), tally)| Check {
            function,
            reference,
            inputs: "all 2^32 floats".into(),
            tally,
        })
        .collect();
    let failing = checks[..3]
        .iter()
        .filter(|c| c.tally.same != c.tally.n)
        .count();
    let builds_differ = checks[3].tally.n - checks[3].tally.same;
    let commit = std::env::var("LARMORX_COMMIT").unwrap_or_else(|_| "unknown".into());
    let result = format!(
        "{{\n  \"larmorx_commit\": \"{commit}\",\n  \"libc\": \"glibc {}\",\n  \"glibc_tunables\": \"{tunables}\",\n  \
         \"threads\": {threads},\n  \"seconds\": {:.1},\n  \"checks\": [\n{}  ],\n  \
         \"failing_checks\": {failing},\n  \"inputs_where_the_builds_differ\": {builds_differ}\n}}\n",
        libc_version(),
        start.elapsed().as_secs_f64(),
        json_checks(&checks),
    );
    print!("{result}");
    if let Some(path) = out {
        std::fs::write(path, &result).expect("the result file");
    }
    // The system must have run its SSE2 build: the FMA build then differs on 34 inputs.
    let ok = failing == 0 && builds_differ == 34;
    println!("{}", if ok { "SSE2-PASS" } else { "SSE2-FAIL" });
    if !ok {
        std::process::exit(1);
    }
}

/// Every float input of the one-argument float functions.
fn float_checks(m: &Libm, threads: usize, checks: &mut Vec<Check>) {
    let names: [(&'static str, &'static str); 16] = [
        ("sinf_glibc", "glibc sinf"),
        ("cosf_glibc", "glibc cosf"),
        ("sincosf_glibc (sine)", "glibc sincosf"),
        ("sincosf_glibc (cosine)", "glibc sincosf"),
        ("logf_glibc", "glibc logf"),
        ("expf_glibc", "glibc expf"),
        ("atanf_glibc", "glibc atanf"),
        ("tanhf_glibc", "glibc tanhf"),
        ("expm1f_glibc", "glibc expm1f"),
        (
            "acosf_glibc",
            "glibc acosf (acosf@@GLIBC_2.2.5, its SVID wrapper)",
        ),
        (
            "sinf_glibc",
            "larmorx-afni-replica sincosf::sinf (the earlier translation)",
        ),
        (
            "cosf_glibc",
            "larmorx-afni-replica sincosf::cosf (the earlier translation)",
        ),
        (
            "sinf_glibc_sse2",
            "larmorx-afni-replica sincosf::sse2::sinf (the earlier translation)",
        ),
        (
            "cosf_glibc_sse2",
            "larmorx-afni-replica sincosf::sse2::cosf (the earlier translation)",
        ),
        (
            "sincosf_glibc_sse2",
            "sinf_glibc_sse2 and cosf_glibc_sse2 (both outputs)",
        ),
        (
            "sinf_glibc_sse2 and cosf_glibc_sse2",
            "sinf_glibc and cosf_glibc: where glibc's two builds differ",
        ),
    ];
    let t = parallel(threads, (1u64 << 32) / CHUNK, names.len(), |c| {
        let mut v: Vec<Tally> = (0..names.len()).map(|_| Tally::default()).collect();
        for i in c * CHUNK..(c + 1) * CHUNK {
            let x = f32::from_bits(i as u32);
            let what = || format!("{i:#010x}");
            let (gs, gc) = m.sincos(x);
            let pairs: [(f32, f32); 14] = [
                ((m.sinf)(x), lx::sinf_glibc(x)),
                ((m.cosf)(x), lx::cosf_glibc(x)),
                (gs, lx::sincosf_glibc(x).0),
                (gc, lx::sincosf_glibc(x).1),
                ((m.logf)(x), lx::logf_glibc(x)),
                ((m.expf)(x), lx::expf_glibc(x)),
                ((m.atanf)(x), lx::atanf_glibc(x)),
                ((m.tanhf)(x), lx::tanhf_glibc(x)),
                ((m.expm1f)(x), lx::expm1f_glibc(x)),
                ((m.acosf)(x), lx::acosf_glibc(x)),
                (afni::sinf(x), lx::sinf_glibc(x)),
                (afni::cosf(x), lx::cosf_glibc(x)),
                (afni::sse2::sinf(x), lx::sinf_glibc_sse2(x)),
                (afni::sse2::cosf(x), lx::cosf_glibc_sse2(x)),
            ];
            for (k, (r, o)) in pairs.into_iter().enumerate() {
                v[k].count(b32(r), r.is_nan(), b32(o), what);
            }
            let (s2, c2) = lx::sincosf_glibc_sse2(x);
            let (rs, rc) = (lx::sinf_glibc_sse2(x), lx::cosf_glibc_sse2(x));
            v[14].count(
                b32(rs) << 32 | b32(rc),
                rs.is_nan(),
                b32(s2) << 32 | b32(c2),
                what,
            );
            // Where the SSE2 build differs from the FMA build (a count, not a failure).
            v[15].count(
                b32(lx::sinf_glibc(x)) << 32 | b32(lx::cosf_glibc(x)),
                false,
                b32(rs) << 32 | b32(rc),
                what,
            );
        }
        v
    });
    for ((function, reference), tally) in names.into_iter().zip(t) {
        checks.push(Check {
            function,
            reference,
            inputs: "all 2^32 floats".into(),
            tally,
        });
    }
}

/// Sampled families and every pair (or every value) of the special values, for one function.
#[allow(clippy::too_many_arguments)]
fn sampled<T: Copy + Sync>(
    threads: usize,
    g: u64,
    families: &[Family<T>],
    specials: &[T],
    function: &'static str,
    reference: &'static str,
    compare: impl Fn(T, &mut Tally) + Sync,
    checks: &mut Vec<Check>,
) {
    for (f, fam) in families.iter().enumerate() {
        let t = parallel(threads, fam.count / CHUNK, 1, |c| {
            let mut t = Tally::default();
            for i in c * CHUNK..(c + 1) * CHUNK {
                compare((fam.arg)(random(g, f as u64, i)), &mut t);
            }
            vec![t]
        });
        checks.push(Check {
            function,
            reference,
            inputs: format!("{} ({} arguments)", fam.name, fam.count),
            tally: t.into_iter().next().unwrap(),
        });
    }
    let mut t = Tally::default();
    for &a in specials {
        compare(a, &mut t);
    }
    checks.push(Check {
        function,
        reference,
        inputs: format!("special values ({})", specials.len()),
        tally: t,
    });
}

fn pairs_of<T: Copy>(v: &[T]) -> Vec<(T, T)> {
    v.iter()
        .flat_map(|&a| v.iter().map(move |&b| (a, b)))
        .collect()
}

fn pair_checks(m: &Libm, threads: usize, checks: &mut Vec<Check>) {
    let s = pairs_of(&samples::special_f32());
    sampled(
        threads,
        1,
        &samples::POWF,
        &s,
        "powf_glibc",
        "glibc powf",
        |(x, y), t| {
            let r = (m.powf)(x, y);
            t.count(b32(r), r.is_nan(), b32(lx::powf_glibc(x, y)), || {
                format!("({:#010x}, {:#010x})", x.to_bits(), y.to_bits())
            });
        },
        checks,
    );
    sampled(
        threads,
        2,
        &samples::ATAN2F,
        &s,
        "atan2f_glibc",
        "glibc atan2f",
        |(y, x), t| {
            let r = (m.atan2f)(y, x);
            t.count(b32(r), r.is_nan(), b32(lx::atan2f_glibc(y, x)), || {
                format!("({:#010x}, {:#010x})", y.to_bits(), x.to_bits())
            });
        },
        checks,
    );
}

fn double_checks(m: &Libm, threads: usize, checks: &mut Vec<Check>) {
    let s = samples::special_f64();
    sampled(
        threads,
        3,
        &samples::EXP,
        &s,
        "exp_glibc",
        "glibc exp",
        |x, t| {
            let r = (m.exp)(x);
            t.count(r.to_bits(), r.is_nan(), lx::exp_glibc(x).to_bits(), || {
                format!("{:#018x}", x.to_bits())
            });
        },
        checks,
    );
    sampled(
        threads,
        4,
        &samples::LOG,
        &s,
        "log_glibc",
        "glibc log",
        |x, t| {
            let r = (m.log)(x);
            t.count(r.to_bits(), r.is_nan(), lx::log_glibc(x).to_bits(), || {
                format!("{:#018x}", x.to_bits())
            });
        },
        checks,
    );
    let sp = pairs_of(&s);
    sampled(
        threads,
        5,
        &samples::POW,
        &sp,
        "pow_glibc",
        "glibc pow",
        |(x, y), t| {
            let r = (m.pow)(x, y);
            t.count(
                r.to_bits(),
                r.is_nan(),
                lx::pow_glibc(x, y).to_bits(),
                || format!("({:#018x}, {:#018x})", x.to_bits(), y.to_bits()),
            );
        },
        checks,
    );
}

const FNV_START: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0100_0000_01b3;

#[inline]
fn fnv(h: u64, v: u64) -> u64 {
    (h ^ v).wrapping_mul(FNV_PRIME)
}

/// A double uniform in [0, 1) from the top 53 bits (as the unit tests compute it).
fn unit(r: u64) -> f64 {
    (r >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

/// The hashes the unit tests of `larmorx_core::math` check, from the system's results: every
/// 4099th float input (FNV-1a over the 32-bit results in input order), and 65,536 seeded
/// arguments of the other functions (FNV-1a over the results' bits; the arguments as the unit
/// tests make them, SplitMix64 of the index, the even ones random bit patterns, the odd ones in
/// the functions' common ranges).
fn hashes(m: &Libm) {
    let mut h = [FNV_START; 6];
    let mut x = 0u64;
    while x <= u64::from(u32::MAX) {
        let y = f32::from_bits(x as u32);
        for (k, v) in [
            (m.sinf)(y),
            (m.cosf)(y),
            (m.logf)(y),
            (m.expf)(y),
            (m.atanf)(y),
            m.sincos(y).1,
        ]
        .into_iter()
        .enumerate()
        {
            h[k] = fnv(h[k], b32(v));
        }
        x += 4099;
    }
    println!(
        "stride 4099: sinf {:#018x} cosf {:#018x} logf {:#018x} expf {:#018x} atanf {:#018x} sincosf-cos {:#018x}",
        h[0], h[1], h[2], h[3], h[4], h[5]
    );
    // tanhf, expm1f, acosf (2026-11-13), the same stride.
    let mut h3 = [FNV_START; 3];
    let mut x = 0u64;
    while x <= u64::from(u32::MAX) {
        let y = f32::from_bits(x as u32);
        for (k, v) in [(m.tanhf)(y), (m.expm1f)(y), (m.acosf)(y)]
            .into_iter()
            .enumerate()
        {
            h3[k] = fnv(h3[k], b32(v));
        }
        x += 4099;
    }
    println!(
        "stride 4099: tanhf {:#018x} expm1f {:#018x} acosf {:#018x}",
        h3[0], h3[1], h3[2]
    );
    // The SSE2 build cannot be called on a CPU with FMA (glibc picks the FMA build); its hash is
    // larmorx's, which the AFNI replica's earlier translation fixes (`verify float`).
    let mut s2 = FNV_START;
    let mut x = 0u64;
    while x <= u64::from(u32::MAX) {
        s2 = fnv(s2, b32(lx::sinf_glibc_sse2(f32::from_bits(x as u32))));
        x += 4099;
    }
    println!("stride 4099: sinf_glibc_sse2 (larmorx's) {s2:#018x}");
    let mut hd = [FNV_START; 5];
    for k in 0..65_536u64 {
        let r = splitmix64(k);
        let r2 = splitmix64(r);
        let odd = k & 1 == 1;
        let ex = if odd {
            unit(r) * 1500.0 - 750.0
        } else {
            f64::from_bits(r)
        };
        let lg = if odd {
            unit(r) * 0.2 + 0.9
        } else {
            f64::from_bits(r & 0x7fff_ffff_ffff_ffff)
        };
        let (px, py) = if odd {
            (unit(r) * 1e3, unit(r2) * 8.0 - 4.0)
        } else {
            (f64::from_bits(r), f64::from_bits(r2))
        };
        let (fx, fy) = if odd {
            (unit(r) as f32 * 100.0, unit(r2) as f32 * 20.0 - 10.0)
        } else {
            (f32::from_bits(r as u32), f32::from_bits((r >> 32) as u32))
        };
        let (ay, ax) = if odd {
            (unit(r) as f32 * 2.0 - 1.0, unit(r2) as f32 * 2.0 - 1.0)
        } else {
            (f32::from_bits(r as u32), f32::from_bits((r >> 32) as u32))
        };
        hd[0] = fnv(hd[0], (m.exp)(ex).to_bits());
        hd[1] = fnv(hd[1], (m.log)(lg).to_bits());
        hd[2] = fnv(hd[2], (m.pow)(px, py).to_bits());
        hd[3] = fnv(hd[3], b32((m.powf)(fx, fy)));
        hd[4] = fnv(hd[4], b32((m.atan2f)(ay, ax)));
    }
    println!(
        "65536 seeded: exp {:#018x} log {:#018x} pow {:#018x} powf {:#018x} atan2f {:#018x}",
        hd[0], hd[1], hd[2], hd[3], hd[4]
    );
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let mut out = None;
    if let Some(i) = args.iter().position(|a| a == "--threads") {
        threads = args[i + 1].parse().expect("--threads N");
        args.drain(i..i + 2);
    }
    if let Some(i) = args.iter().position(|a| a == "--out") {
        out = Some(args[i + 1].clone());
        args.drain(i..i + 2);
    }
    let m = Libm::open();
    let command = if args.is_empty() {
        "verify".to_string()
    } else {
        args.remove(0)
    };
    if command == "hashes" {
        hashes(&m);
        return;
    }
    if command == "sse2" {
        sse2(&m, threads, out.as_deref());
        return;
    }
    assert_eq!(command, "verify", "commands: verify, sse2, hashes");
    let all = args.is_empty();
    let want = |s: &str| all || args.iter().any(|a| a == s);
    #[cfg(target_arch = "x86_64")]
    let fma =
        std::arch::is_x86_feature_detected!("fma") && std::arch::is_x86_feature_detected!("avx2");
    #[cfg(not(target_arch = "x86_64"))]
    let fma = false;
    let start = Instant::now();
    let mut checks = Vec::new();
    if want("float") {
        float_checks(&m, threads, &mut checks);
        eprintln!("float: {:.1} s", start.elapsed().as_secs_f64());
    }
    if want("pairs") {
        pair_checks(&m, threads, &mut checks);
        eprintln!("pairs: {:.1} s", start.elapsed().as_secs_f64());
    }
    if want("double") {
        double_checks(&m, threads, &mut checks);
        eprintln!("double: {:.1} s", start.elapsed().as_secs_f64());
    }
    // The count of the SSE2 build's differences is informative, not a failure.
    let failing: Vec<&Check> = checks
        .iter()
        .filter(|c| {
            !c.reference.starts_with("sinf_glibc and cosf_glibc") && c.tally.same != c.tally.n
        })
        .collect();
    // The larmorx commit the tool was built from (`LARMORX_COMMIT`, set by the caller).
    let commit = std::env::var("LARMORX_COMMIT").unwrap_or_else(|_| "unknown".into());
    let result = format!(
        "{{\n  \"larmorx_commit\": \"{commit}\",\n  \"libc\": \"glibc {}\",\n  \"machine\": \"{}\",\n  \
         \"cpu_has_fma_and_avx2\": {fma},\n  \"threads\": {threads},\n  \"seconds\": {:.1},\n  \
         \"checks\": [\n{}  ],\n  \"failing_checks\": {}\n}}\n",
        libc_version(),
        std::env::consts::ARCH,
        start.elapsed().as_secs_f64(),
        json_checks(&checks),
        failing.len()
    );
    print!("{result}");
    if let Some(path) = out {
        std::fs::write(path, &result).expect("the result file");
    }
    let total: u64 = checks.iter().map(|c| c.tally.n).sum();
    println!(
        "{} checks, {total} comparisons, {} failing",
        checks.len(),
        failing.len()
    );
    println!(
        "{}",
        if failing.is_empty() {
            "VERIFY-PASS"
        } else {
            "VERIFY-FAIL"
        }
    );
    if !failing.is_empty() {
        std::process::exit(1);
    }
}
