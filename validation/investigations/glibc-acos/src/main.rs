// SPDX-License-Identifier: Apache-2.0
//! glibc 2.35's double `acos`, called from the C library's `libm.so.6` (opened with `dlopen`: a
//! black box; glibc's code is never read here), against `larmorx_core::math::acos` (a port of
//! CORE-MATH's correctly rounded `cr_acos`, proved bit-identical to the C on all of `acos.wc`).
//!
//! `glibc-acos-blackbox [--threads N] [--log2n K] --wc <acos.wc> [--out record.json]
//! [--list diffs.txt]`
//!
//! The families (random numbers: SplitMix64 of the state `(0xac << 56) | (family << 44) | i`):
//!
//! 1. uniform on [-1, 1): `fl(-1 + 2u)`, `u = (r >> 11) 2^-53`; 2^K arguments (K = 32 by default);
//! 2. every binade of `|x| < 1`: `r`'s sign bit and 52 low bits, the exponent field
//!    `((r >> 52) AND 0x7ff) mod 1023` (subnormals included); 2^K arguments;
//! 3. subnormals: `r`'s sign bit and 52 low bits, exponent field 0; 2^(K - 4) arguments;
//! 4. neighbourhoods: the 2^25 consecutive bit patterns around each centre (half below, half
//!    above; from +0 the first 2^25), each with both signs: the centres 0, 0.5, 1 and the limits of
//!    `specs/glibc-dbl.md` §10.1 (where glibc's paths change, observed);
//! 5. CORE-MATH's worst cases (`acos.wc`, the hard cases of correct rounding): each argument's
//!    magnitude and its 16 neighbours on each side, with both signs (sorted, duplicates removed);
//! 6. special arguments (zeros, subnormal and normal limits, ±0.5, ±1 and their neighbours, ±2,
//!    the largest double, infinities, quiet and signalling NaNs with payloads).
//!
//! For each family: how many results differ (values; NaN results compared by their bits apart),
//! by how many units in the last place, the hash of glibc's results (FNV-1a on the 64 result bits
//! in argument order, in blocks of 2^22, a family's hash over its block hashes: the glibc-dbl
//! record's rule), and the arguments where they differ (all of them for families 4 to 6, the
//! first 16 for the others, into `--list`). Families 1 and 2 also give the differences per binade.

use std::collections::BTreeMap;
use std::ffi::{CStr, c_char, c_int, c_void};
use std::fmt::Write as _;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use larmorx_core::math as lx;

unsafe extern "C" {
    fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlerror() -> *const c_char;
    fn gnu_get_libc_version() -> *const c_char;
}

type F64 = extern "C" fn(f64) -> f64;

fn glibc_acos() -> F64 {
    // SAFETY: plain calls of the C library's dynamic loader; the symbol is cast to its C
    // signature `double acos(double)`.
    unsafe {
        let h = dlopen(c"libm.so.6".as_ptr(), 2);
        if h.is_null() {
            panic!("dlopen(libm.so.6): {:?}", CStr::from_ptr(dlerror()));
        }
        let f = dlsym(h, c"acos".as_ptr());
        if f.is_null() {
            panic!("dlsym(acos): {:?}", CStr::from_ptr(dlerror()));
        }
        std::mem::transmute::<*mut c_void, F64>(f)
    }
}

const BLOCK: u64 = 1 << 22;
const SIGN: u64 = 0x8000_0000_0000_0000;
const MANT: u64 = 0x000f_ffff_ffff_ffff;
const FNV0: u64 = 14_695_981_039_346_656_037;
const FNVP: u64 = 1_099_511_628_211;
const HALF: u64 = 1 << 24;

/// The limits of `specs/glibc-dbl.md` §10.1 for `asin` and `acos` (positive bit patterns), with
/// 0 and 0.5 (1 = `0x3ff0…` is among them).
const CENTRES: [u64; 11] = [
    0,
    0x3c88_0000_0000_0000,
    0x3e50_0000_0000_0000,
    0x3fc0_0000_0000_0000,
    0x3fd0_0000_0000_0000,
    0x3fe0_0000_0000_0000,
    0x3fe8_0000_0000_0000,
    0x3fed_8000_0000_0000,
    0x3fee_8000_0000_0000,
    0x3fef_0000_0000_0000,
    0x3ff0_0000_0000_0000,
];

fn splitmix64(s: u64) -> u64 {
    let mut z = s.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

fn random(f: u64, i: u64) -> u64 {
    splitmix64((0xac << 56) | (f << 44) | i)
}

/// Parses one line of CORE-MATH's `.wc` files: `[+-]0x<h>[.<h>]p<e>`, `[+-]0x.<h>p<e>`,
/// `[+-]0`, `[+-]inf`, `[+-]nan`, `[+-]snan`. Every value there is a double: an argument that
/// would need rounding is an error.
fn parse_wc(line: &str) -> Option<u64> {
    let t = line.split_whitespace().next()?;
    if t.starts_with('#') {
        return None;
    }
    let (sign, body) = match t.as_bytes()[0] {
        b'-' => (SIGN, &t[1..]),
        b'+' => (0, &t[1..]),
        _ => (0, t),
    };
    let mag = match body {
        "inf" => 0x7ff0_0000_0000_0000,
        "nan" => 0x7ff8_0000_0000_0000,
        "snan" => 0x7ff4_0000_0000_0000,
        "0" => 0,
        _ if body.bytes().all(|c| c.is_ascii_digit()) => {
            let v: u32 = body.parse().unwrap_or_else(|_| panic!("bad integer: {t}"));
            f64::from(v).to_bits()
        }
        _ => {
            let hex = body.strip_prefix("0x").unwrap_or_else(|| panic!("not a hex float: {t}"));
            let (mant, exp) = hex.split_once('p').unwrap_or_else(|| panic!("no exponent: {t}"));
            let e: i64 = exp.parse().unwrap_or_else(|_| panic!("bad exponent: {t}"));
            let (int, frac) = mant.split_once('.').unwrap_or((mant, ""));
            let mut m: u128 = 0;
            for c in int.chars().chain(frac.chars()) {
                m = (m << 4) | u128::from(c.to_digit(16).unwrap_or_else(|| panic!("digit: {t}")));
            }
            let mut e2 = e - 4 * frac.len() as i64;
            if m == 0 {
                0
            } else {
                while m >> 53 != 0 {
                    assert!(m & 1 == 0, "not a double: {t}");
                    m >>= 1;
                    e2 += 1;
                }
                // m * 2^e2 exactly: normalise to 53 bits, then the exponent field.
                while m >> 52 == 0 {
                    m <<= 1;
                    e2 -= 1;
                }
                let biased = e2 + 52 + 1023;
                if biased >= 1 {
                    assert!(biased < 2047, "overflow: {t}");
                    ((biased as u64) << 52) | (m as u64 & MANT)
                } else {
                    let shift = 1 - biased;
                    assert!(shift <= 53, "underflow: {t}");
                    assert!(m & ((1u128 << shift) - 1) == 0, "not a double: {t}");
                    (m >> shift) as u64
                }
            }
        }
    };
    Some(sign | mag)
}

enum Args {
    Uniform,
    Binades,
    Subnormals,
    Neighbourhoods,
    List(Vec<u64>),
}

struct Family {
    id: u64,
    name: &'static str,
    n: u64,
    args: Args,
    keep_all: bool,
}

impl Family {
    fn arg(&self, i: u64) -> u64 {
        match &self.args {
            Args::Uniform => {
                let u = (random(self.id, i) >> 11) as f64 * f64::from_bits(0x3ca0_0000_0000_0000);
                (-1.0 + 2.0 * u).to_bits()
            }
            Args::Binades => {
                let r = random(self.id, i);
                (r & SIGN) | ((((r >> 52) & 0x7ff) % 1023) << 52) | (r & MANT)
            }
            Args::Subnormals => {
                let r = random(self.id, i);
                (r & SIGN) | (r & MANT)
            }
            Args::Neighbourhoods => {
                let c = CENTRES[(i >> 26) as usize];
                let j = i & ((1 << 26) - 1);
                let sign = if j & 1 == 1 { SIGN } else { 0 };
                let p = if c == 0 { j >> 1 } else { c - HALF + (j >> 1) };
                sign | p
            }
            Args::List(v) => v[i as usize],
        }
    }
}

#[derive(Default, Clone)]
struct Tally {
    n: u64,
    value: u64,
    nan_bits: u64,
    nan_one: u64,
    ulps: BTreeMap<u64, u64>,
    diffs: Vec<[u64; 4]>,
    /// Per binade (the biased exponent of `|x|`): arguments, differing results.
    binades: BTreeMap<u64, (u64, u64)>,
    hash: u64,
}

impl Tally {
    fn merge(&mut self, o: Tally, keep_all: bool) {
        self.n += o.n;
        self.value += o.value;
        self.nan_bits += o.nan_bits;
        self.nan_one += o.nan_one;
        for (k, v) in o.ulps {
            *self.ulps.entry(k).or_default() += v;
        }
        for d in o.diffs {
            if keep_all || self.diffs.len() < 16 {
                self.diffs.push(d);
            }
        }
        for (k, (a, b)) in o.binades {
            let e = self.binades.entry(k).or_default();
            e.0 += a;
            e.1 += b;
        }
    }
}

fn run_block(f: &Family, g: F64, b: u64) -> Tally {
    let mut t = Tally { hash: FNV0, ..Tally::default() };
    let end = ((b + 1) * BLOCK).min(f.n);
    let binades = matches!(f.args, Args::Uniform | Args::Binades);
    for i in b * BLOCK..end {
        let xb = f.arg(i);
        let x = f64::from_bits(xb);
        let gv = g(x);
        let cv = lx::acos(x);
        let (gb, cb) = (gv.to_bits(), cv.to_bits());
        t.n += 1;
        t.hash = (t.hash ^ gb).wrapping_mul(FNVP);
        let differs = if gv.is_nan() && cv.is_nan() {
            if gb != cb {
                t.nan_bits += 1;
            }
            gb != cb
        } else if gv.is_nan() != cv.is_nan() {
            t.nan_one += 1;
            true
        } else if gb != cb {
            t.value += 1;
            *t.ulps.entry(gb.abs_diff(cb)).or_default() += 1;
            true
        } else {
            false
        };
        if binades {
            let e = t.binades.entry((xb >> 52) & 0x7ff).or_default();
            e.0 += 1;
            if differs && !(gv.is_nan() && cv.is_nan()) {
                e.1 += 1;
            }
        }
        if differs && (f.keep_all || t.diffs.len() < 16) {
            t.diffs.push([i, xb, gb, cb]);
        }
    }
    t
}

fn run_family(f: &Family, g: F64, threads: usize) -> Tally {
    let blocks = f.n.div_ceil(BLOCK);
    let next = AtomicU64::new(0);
    let out: Mutex<Vec<(u64, Tally)>> = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let b = next.fetch_add(1, Ordering::Relaxed);
                    if b >= blocks {
                        break;
                    }
                    let t = run_block(f, g, b);
                    out.lock().unwrap().push((b, t));
                }
            });
        }
    });
    let mut parts = out.into_inner().unwrap();
    parts.sort_by_key(|p| p.0);
    let mut total = Tally { hash: FNV0, ..Tally::default() };
    for (_, t) in parts {
        total.hash = (total.hash ^ t.hash).wrapping_mul(FNVP);
        total.merge(t, f.keep_all);
    }
    total
}

fn specials() -> Vec<u64> {
    let pos: [u64; 18] = [
        0,
        1,
        MANT,
        0x0010_0000_0000_0000,
        0x3fe0_0000_0000_0000,
        0x3fdf_ffff_ffff_ffff,
        0x3fe0_0000_0000_0001,
        0x3fef_ffff_ffff_ffff,
        0x3ff0_0000_0000_0000,
        0x3ff0_0000_0000_0001,
        0x4000_0000_0000_0000,
        0x7fef_ffff_ffff_ffff,
        0x7ff0_0000_0000_0000,
        0x7ff8_0000_0000_0000,
        0x7ff8_0000_0000_0123,
        0x7ff4_0000_0000_0000,
        0x7ff0_0000_0000_0001,
        0x7fff_ffff_ffff_ffff,
    ];
    pos.iter().flat_map(|&p| [p, p | SIGN]).collect()
}

fn main() {
    let mut threads = 1usize;
    let mut log2n = 32u32;
    let mut wc: Option<String> = None;
    let mut out: Option<String> = None;
    let mut list: Option<String> = None;
    let a: Vec<String> = std::env::args().skip(1).collect();
    let mut k = 0;
    while k < a.len() {
        let v = a.get(k + 1).cloned().unwrap_or_default();
        match a[k].as_str() {
            "--threads" => threads = v.parse().expect("--threads N"),
            "--log2n" => log2n = v.parse().expect("--log2n K"),
            "--wc" => wc = Some(v),
            "--out" => out = Some(v),
            "--list" => list = Some(v),
            o => panic!("unknown option {o}"),
        }
        k += 2;
    }
    let wc = wc.expect("--wc <core-math>/src/binary64/acos/acos.wc");
    let text = std::fs::read_to_string(&wc).expect("read acos.wc");
    let mut parsed = 0u64;
    let mut cases: Vec<u64> = Vec::new();
    for line in text.lines() {
        let Some(bits) = parse_wc(line) else { continue };
        parsed += 1;
        let mag = bits & !SIGN;
        if mag >= 0x7ff0_0000_0000_0000 {
            cases.push(mag);
            cases.push(mag | SIGN);
            continue;
        }
        for d in -16i64..=16 {
            let p = mag as i64 + d;
            if (0..0x7ff0_0000_0000_0000).contains(&p) {
                cases.push(p as u64);
                cases.push(p as u64 | SIGN);
            }
        }
    }
    cases.sort_unstable();
    cases.dedup();
    let n = 1u64 << log2n;
    let families = vec![
        Family { id: 1, name: "uniform on [-1, 1)", n, args: Args::Uniform, keep_all: false },
        Family { id: 2, name: "every binade of |x| < 1", n, args: Args::Binades, keep_all: false },
        Family { id: 3, name: "subnormals", n: n >> 4, args: Args::Subnormals, keep_all: false },
        Family {
            id: 4,
            name: "neighbourhoods of 0, 0.5, 1 and the limits",
            n: (CENTRES.len() as u64) << 26,
            args: Args::Neighbourhoods,
            keep_all: true,
        },
        Family {
            id: 5,
            name: "CORE-MATH's worst cases and their neighbours",
            n: cases.len() as u64,
            args: Args::List(cases),
            keep_all: true,
        },
        Family {
            id: 6,
            name: "special arguments",
            n: specials().len() as u64,
            args: Args::List(specials()),
            keep_all: true,
        },
    ];
    let g = glibc_acos();
    // SAFETY: gnu_get_libc_version returns a static NUL-terminated string.
    let version = unsafe { CStr::from_ptr(gnu_get_libc_version()) }.to_string_lossy();
    let tunables = std::env::var("GLIBC_TUNABLES").unwrap_or_default();
    println!(
        "glibc {version}, GLIBC_TUNABLES='{tunables}', {threads} threads; acos.wc: {parsed} arguments"
    );
    let mut json = String::new();
    let _ = write!(
        json,
        "{{\n  \"what\": \"glibc's double acos as a black box against larmorx_core::math::acos (CORE-MATH's cr_acos)\",\n  \"glibc\": \"{version}\",\n  \"tunables\": \"{tunables}\",\n  \"log2n\": {log2n},\n  \"families\": ["
    );
    let mut listing = String::new();
    let mut grand = (0u64, 0u64);
    for (fi, f) in families.iter().enumerate() {
        let t0 = Instant::now();
        let t = run_family(f, g, threads);
        let secs = t0.elapsed().as_secs_f64();
        let differ = t.value + t.nan_one;
        grand.0 += t.n;
        grand.1 += differ;
        let ulps: Vec<String> = t.ulps.iter().map(|(k, v)| format!("{k} ulp: {v}")).collect();
        println!(
            "family {} ({}): {} arguments, {} results differ ({:.4} %; {}), NaN bits differ {}, NaN against a number {}, hash {:016x}, {:.1} s",
            f.id,
            f.name,
            t.n,
            differ,
            100.0 * differ as f64 / t.n as f64,
            if ulps.is_empty() { "none".to_string() } else { ulps.join(", ") },
            t.nan_bits,
            t.nan_one,
            t.hash,
            secs
        );
        if !t.binades.is_empty() {
            let mut line = String::from("  per binade (2^e: differing / arguments):");
            for (e, (a, d)) in &t.binades {
                if *d > 0 {
                    let ex = if *e == 0 { -1074 } else { *e as i64 - 1023 };
                    let _ = write!(line, " 2^{ex}: {d}/{a}");
                }
            }
            println!("{line}");
            let with: Vec<i64> = t
                .binades
                .iter()
                .filter(|(_, (_, d))| *d > 0)
                .map(|(e, _)| if *e == 0 { -1074 } else { *e as i64 - 1023 })
                .collect();
            let all = t.binades.values().filter(|(a, _)| *a > 0).count();
            if let (Some(lo), Some(hi)) = (with.first(), with.last()) {
                println!(
                    "  differences in {} of {all} binades, from 2^{lo} to 2^{hi}",
                    with.len()
                );
            }
        }
        for d in t.diffs.iter().take(if f.keep_all { usize::MAX } else { 16 }) {
            let _ = writeln!(
                listing,
                "{} {} {:016x} {:016x} {:016x} {:e}",
                f.id,
                d[0],
                d[1],
                d[2],
                d[3],
                f64::from_bits(d[1])
            );
        }
        let _ = write!(
            json,
            "{}\n    {{\"family\": {}, \"name\": \"{}\", \"arguments\": {}, \"differ\": {}, \"nan_bits_differ\": {}, \"hash\": \"{:016x}\"}}",
            if fi == 0 { "" } else { "," },
            f.id,
            f.name,
            t.n,
            differ,
            t.nan_bits,
            t.hash
        );
    }
    json.push_str("\n  ]\n}\n");
    println!(
        "ALL: {} arguments, {} results differ ({:.4} %)",
        grand.0,
        grand.1,
        100.0 * grand.1 as f64 / grand.0 as f64
    );
    if let Some(p) = out {
        std::fs::write(&p, json).expect("write --out");
    }
    if let Some(p) = list {
        std::fs::write(
            &p,
            format!("# family index argument-bits glibc-bits correctly-rounded-bits argument\n{listing}"),
        )
        .expect("write --list");
    }
}
