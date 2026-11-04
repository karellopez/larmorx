// SPDX-License-Identifier: Apache-2.0
//! The clean-room `sin_glibc`, `cos_glibc` and `sincos_glibc` of `larmorx_core::math`
//! (`specs/glibc-dbl.md`) against glibc 2.35, bit for bit (NaN results included, no canonical
//! NaN). glibc's code is never read: the record is black-box output, and the system's functions
//! are called through `dlopen` from `libm.so.6`.
//!
//! - `record --record <json> [--sse2] [--threads N] [--out result.json]`: the spec's §12.1. The
//!   block hashes of §10.2 from larmorx's functions on every family of `families.rs` (2^27
//!   arguments each, blocks of 2^22), compared with the record's blocks, and the special
//!   arguments one by one. `--sse2`: `sin_glibc_sse2` and `cos_glibc_sse2` against the record of
//!   glibc's build without FMA (§11.2, `glibc-dbl-2.35-x86_64-sse2-blocks.json`).
//! - `fresh [--sse2] [--log2n K] [--threads N] [--out result.json]`: larmorx against the system's
//!   functions on fresh arguments: every family of `families.rs` at the indices after the
//!   record's (from 2^27), and three families of this tool's own (the table rows' neighbourhoods,
//!   the band near π/2, the medium reduction's range), 2^K arguments each. `--sse2` must run with
//!   `GLIBC_TUNABLES=glibc.cpu.hwcaps=-AVX2,-FMA` (glibc's build without FMA; `sincos` has one
//!   build).
//! - `hashes [--sse2]`: the hashes the unit tests of `larmorx_core::math::sincos_glibc` check,
//!   from the system's functions (the unit tests' 65,536 seeded arguments).
//! - `bench [--log2n K]`: nanoseconds per call, one thread: larmorx's `*_glibc`, its correctly
//!   rounded `sin` and `cos` (CORE-MATH) and the system's functions, in three ranges.
//! - `list <function> <family> <first> <count> [--sse2]`: larmorx's results, in the format of
//!   `glibc-dbl/blackbox list` (`index argument-bits result-bits`), to compare inside a block.

#[path = "../../../glibc-dbl/families.rs"]
#[allow(dead_code)]
mod families;

use std::ffi::{CStr, c_char, c_int, c_void};
use std::fmt::Write as _;
use std::hint::black_box;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use families::{BLOCK, COUNT, FUNCTIONS, families, random, special, splitmix64};
use larmorx_core::math as lx;

unsafe extern "C" {
    fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlerror() -> *const c_char;
    fn gnu_get_libc_version() -> *const c_char;
}

type F64 = extern "C" fn(f64) -> f64;
type SinCos = extern "C" fn(f64, *mut f64, *mut f64);

struct Libm {
    sin: F64,
    cos: F64,
    sincos: SinCos,
}

impl Libm {
    fn open() -> Libm {
        fn symbol(h: *mut c_void, name: &CStr) -> *mut c_void {
            // SAFETY: a plain call of the dynamic loader with a NUL-terminated name.
            let f = unsafe { dlsym(h, name.as_ptr()) };
            assert!(!f.is_null(), "dlsym({name:?}): {:?}", unsafe {
                // SAFETY: dlerror returns a static NUL-terminated string after a failed call.
                CStr::from_ptr(dlerror())
            });
            f
        }
        // SAFETY: plain calls of the C library's dynamic loader; every symbol is cast to its C
        // signature (`double sin(double)`, `void sincos(double, double *, double *)`).
        unsafe {
            let h = dlopen(c"libm.so.6".as_ptr(), 2);
            assert!(
                !h.is_null(),
                "dlopen(libm.so.6): {:?}",
                CStr::from_ptr(dlerror())
            );
            use std::mem::transmute as t;
            Libm {
                sin: t::<*mut c_void, F64>(symbol(h, c"sin")),
                cos: t::<*mut c_void, F64>(symbol(h, c"cos")),
                sincos: t::<*mut c_void, SinCos>(symbol(h, c"sincos")),
            }
        }
    }

    /// glibc's result bits (`sincos`: the sine's, then the cosine's).
    fn eval(&self, f: &str, x: f64) -> [u64; 2] {
        match f {
            "sin" => [(self.sin)(x).to_bits(), 0],
            "cos" => [(self.cos)(x).to_bits(), 0],
            "sincos" => {
                let (mut s, mut c) = (0.0f64, 0.0f64);
                (self.sincos)(x, &mut s, &mut c);
                [s.to_bits(), c.to_bits()]
            }
            _ => unreachable!(),
        }
    }
}

/// larmorx's result bits: the FMA build (`sse2 = false`) or the build without FMA.
#[inline]
fn ours(f: &str, sse2: bool, x: f64) -> [u64; 2] {
    match (f, sse2) {
        ("sin", false) => [lx::sin_glibc(x).to_bits(), 0],
        ("cos", false) => [lx::cos_glibc(x).to_bits(), 0],
        ("sin", true) => [lx::sin_glibc_sse2(x).to_bits(), 0],
        ("cos", true) => [lx::cos_glibc_sse2(x).to_bits(), 0],
        ("sincos", _) => {
            let (s, c) = lx::sincos_glibc(x);
            [s.to_bits(), c.to_bits()]
        }
        _ => unreachable!(),
    }
}

fn words(f: &str) -> usize {
    if f == "sincos" { 2 } else { 1 }
}

fn stream(f: &str) -> u64 {
    FUNCTIONS.iter().find(|(n, _)| *n == f).expect("function").1
}

const FNV0: u64 = 14_695_981_039_346_656_037;
const FNVP: u64 = 1_099_511_628_211;

#[inline]
fn fnv(h: u64, w: u64) -> u64 {
    (h ^ w).wrapping_mul(FNVP)
}

fn libc_version() -> String {
    // SAFETY: glibc's function returns a static NUL-terminated string.
    unsafe { CStr::from_ptr(gnu_get_libc_version()) }
        .to_string_lossy()
        .into_owned()
}

fn larmorx_commit() -> String {
    std::env::var("LARMORX_COMMIT").unwrap_or_else(|_| "unknown".into())
}

/// Runs `n` jobs on `threads` threads, results in job order.
fn parallel<T: Send + Clone + Default>(
    n: usize,
    threads: usize,
    job: impl Fn(usize) -> T + Sync,
) -> Vec<T> {
    let out: Mutex<Vec<T>> = Mutex::new(vec![T::default(); n]);
    let next = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let w = next.fetch_add(1, Ordering::Relaxed);
                    if w >= n {
                        break;
                    }
                    let r = job(w);
                    out.lock().unwrap()[w] = r;
                }
            });
        }
    });
    out.into_inner().unwrap()
}

// ---- a minimal JSON reader (the record is plain JSON: objects, arrays, strings, numbers) ----

#[derive(Debug)]
enum Json {
    Obj(Vec<(String, Json)>),
    Arr(Vec<Json>),
    Str(String),
    Other,
}

impl Json {
    fn get(&self, k: &str) -> &Json {
        match self {
            Json::Obj(v) => &v.iter().find(|(n, _)| n == k).expect(k).1,
            _ => panic!("not an object at {k}"),
        }
    }
    fn arr(&self) -> &[Json] {
        match self {
            Json::Arr(v) => v,
            _ => panic!("not an array"),
        }
    }
    fn hex(&self) -> u64 {
        match self {
            Json::Str(s) => u64::from_str_radix(s, 16).expect("hex"),
            _ => panic!("not a string"),
        }
    }
}

fn parse(s: &[u8], i: &mut usize) -> Json {
    let ws = |i: &mut usize| {
        while *i < s.len() && s[*i].is_ascii_whitespace() {
            *i += 1;
        }
    };
    ws(i);
    match s[*i] {
        b'{' => {
            *i += 1;
            let mut v = Vec::new();
            loop {
                ws(i);
                if s[*i] == b'}' {
                    *i += 1;
                    break;
                }
                let Json::Str(k) = parse(s, i) else {
                    panic!("key")
                };
                ws(i);
                assert_eq!(s[*i], b':');
                *i += 1;
                v.push((k, parse(s, i)));
                ws(i);
                if s[*i] == b',' {
                    *i += 1;
                }
            }
            Json::Obj(v)
        }
        b'[' => {
            *i += 1;
            let mut v = Vec::new();
            loop {
                ws(i);
                if s[*i] == b']' {
                    *i += 1;
                    break;
                }
                v.push(parse(s, i));
                ws(i);
                if s[*i] == b',' {
                    *i += 1;
                }
            }
            Json::Arr(v)
        }
        b'"' => {
            *i += 1;
            let start = *i;
            while s[*i] != b'"' {
                if s[*i] == b'\\' {
                    *i += 1;
                }
                *i += 1;
            }
            let t = String::from_utf8_lossy(&s[start..*i]).into_owned();
            *i += 1;
            Json::Str(t)
        }
        _ => {
            while *i < s.len() && !matches!(s[*i], b',' | b'}' | b']') {
                *i += 1;
            }
            Json::Other
        }
    }
}

// ---- record: the spec's §12.1 ----

fn record(path: &str, sse2: bool, threads: usize, out: Option<String>) -> bool {
    let text = std::fs::read(path).expect("read the record");
    let rec = parse(&text, &mut 0);
    let fns: &[&str] = if sse2 {
        &["sin", "cos"]
    } else {
        &["sin", "cos", "sincos"]
    };
    let t0 = Instant::now();
    let blocks_per = (COUNT / BLOCK) as usize;
    let mut work = Vec::new();
    for (fi, f) in fns.iter().enumerate() {
        for (ki, _) in families(f).iter().enumerate() {
            for b in 0..blocks_per {
                work.push((fi, ki, b));
            }
        }
    }
    let fams: Vec<_> = fns.iter().map(|f| families(f)).collect();
    let hashes = parallel(work.len(), threads, |w| {
        let (fi, ki, b) = work[w];
        let (f, fam) = (fns[fi], &fams[fi][ki]);
        let g = stream(f);
        let mut h = FNV0;
        for i in (b as u64) * BLOCK..(b as u64 + 1) * BLOCK {
            let r = ours(f, sse2, (fam.arg)(random(g, fam.id, i)));
            for &v in &r[..words(f)] {
                h = fnv(h, v);
            }
        }
        h
    });
    let mut all_ok = true;
    let mut js = String::new();
    let _ = write!(
        js,
        "{{\n  \"what\": \"larmorx_core::math's clean-room sin_glibc, cos_glibc, sincos_glibc{} against the black-box record (specs/glibc-dbl.md §12.1)\",\n  \"record\": \"{}\",\n  \"larmorx_commit\": \"{}\",\n  \"functions\": {{",
        if sse2 { " (sse2 builds)" } else { "" },
        std::path::Path::new(path)
            .file_name()
            .unwrap()
            .to_string_lossy(),
        larmorx_commit()
    );
    let mut w = 0;
    for (fi, f) in fns.iter().enumerate() {
        let rf = rec.get("functions").get(f);
        let mut agree = 0usize;
        let mut differ = Vec::new();
        let mut fam_hashes = Vec::new();
        for (ki, fam) in fams[fi].iter().enumerate() {
            let rfam = &rf.get("families").arr()[ki];
            let want: Vec<u64> = rfam.get("blocks").arr().iter().map(Json::hex).collect();
            let got = &hashes[w..w + blocks_per];
            w += blocks_per;
            for b in 0..blocks_per {
                if got[b] == want[b] {
                    agree += 1;
                } else {
                    differ.push(format!("[{}, {b}]", fam.id));
                }
            }
            let fh = got.iter().fold(FNV0, |h, &b| fnv(h, b));
            assert_eq!(rfam.get("hash").hex() == fh, want == got);
            fam_hashes.push(fh);
        }
        let fn_hash = fam_hashes.iter().fold(FNV0, |h, &b| fnv(h, b));
        // the special arguments, one by one
        let mut sp_ok = 0usize;
        let mut sp_bad = Vec::new();
        let sp = rf.get("special").arr();
        for e in sp {
            let e = e.arr();
            let x = f64::from_bits(e[0].hex());
            let r = ours(f, sse2, x);
            let same = (0..words(f)).all(|k| r[k] == e[k + 1].hex());
            if same {
                sp_ok += 1;
            } else {
                sp_bad.push(format!("\"{:016x}\"", x.to_bits()));
            }
        }
        assert_eq!(sp.len(), special().len());
        let ok = differ.is_empty() && sp_bad.is_empty();
        all_ok &= ok;
        println!(
            "{f:7}{}: blocks {agree} of {} agree; special {sp_ok} of {} agree; function hash {fn_hash:016x}{}",
            if sse2 { " (sse2)" } else { "" },
            fams[fi].len() * blocks_per,
            sp.len(),
            if ok { "" } else { "  DIFFER" }
        );
        if !differ.is_empty() {
            println!("  differing blocks [family, block]: {}", differ.join(" "));
        }
        let _ = write!(
            js,
            "{}\n    \"{f}\": {{\"blocks\": {}, \"blocks_agree\": {agree}, \"special\": {}, \"special_agree\": {sp_ok}, \"function_hash\": \"{fn_hash:016x}\", \"differing_blocks\": [{}], \"differing_special\": [{}]}}",
            if fi > 0 { "," } else { "" },
            fams[fi].len() * blocks_per,
            sp.len(),
            differ.join(", "),
            sp_bad.join(", ")
        );
    }
    let secs = t0.elapsed().as_secs_f64();
    let _ = write!(
        js,
        "\n  }},\n  \"result\": \"{}\",\n  \"seconds\": {secs:.1},\n  \"threads\": {threads}\n}}\n",
        if all_ok { "PASS" } else { "FAIL" }
    );
    println!(
        "{} ({secs:.1} s on {threads} threads)",
        if all_ok { "RECORD-PASS" } else { "RECORD-FAIL" }
    );
    if let Some(p) = out {
        std::fs::write(&p, js).expect("write the result");
    }
    all_ok
}

// ---- fresh: against the system's functions ----

/// This tool's own families (stream numbers 9 to 11, which the record does not use).
fn own_families() -> Vec<families::Family> {
    fn unit(r: u64) -> f64 {
        (r >> 11) as f64 * f64::from_bits(0x3ca0_0000_0000_0000)
    }
    vec![
        families::Family {
            id: 9,
            name: "the table rows: ±(N/128 + u), N = 0 to 109, u uniform on [-1/256, 1/256)",
            arg: |r| {
                let n = (r % 110) as f64 * (1.0 / 128.0);
                let u = unit(splitmix64(r)) * (1.0 / 128.0) - 1.0 / 256.0;
                let x = n + u;
                if r >> 63 == 1 { -x } else { x }
            },
        },
        families::Family {
            id: 10,
            name: "the band near pi/2: uniform on [0.85546875, L1), both signs",
            arg: |r| {
                let l0 = f64::from_bits(0x3feb_6000_0000_0000);
                let l1 = f64::from_bits(0x4003_68fd_0000_0000);
                let x = l0 + (l1 - l0) * unit(r);
                if r & 1 == 1 { -x } else { x }
            },
        },
        families::Family {
            id: 11,
            name: "magnitudes 2^1 to 2^27, log-uniform, both signs (the medium reduction and its limit)",
            arg: |r| {
                let e = 1 + ((r >> 52) & 0x7ff) % 26;
                f64::from_bits(
                    (r & 0x8000_0000_0000_0000) | ((1023 + e) << 52) | (r & 0x000f_ffff_ffff_ffff),
                )
            },
        },
    ]
}

#[derive(Clone, Default)]
struct Fresh {
    compared: u64,
    differ: [u64; 2],
    nan: u64,
    examples: Vec<String>,
}

fn fresh(sse2: bool, log2n: u32, threads: usize, out: Option<String>) -> bool {
    let tunables = std::env::var("GLIBC_TUNABLES").unwrap_or_default();
    if sse2 && !tunables.contains("-FMA") {
        eprintln!("--sse2 needs GLIBC_TUNABLES=glibc.cpu.hwcaps=-AVX2,-FMA");
        std::process::exit(2);
    }
    if !sse2 && tunables.contains("-FMA") {
        eprintln!("GLIBC_TUNABLES switches glibc's FMA builds off: add --sse2");
        std::process::exit(2);
    }
    let m = Libm::open();
    let t0 = Instant::now();
    let fns: &[&str] = if sse2 {
        &["sin", "cos"]
    } else {
        &["sin", "cos", "sincos"]
    };
    let per = 1u64 << log2n;
    let chunk = per.min(BLOCK);
    let chunks = per / chunk;
    let fams: Vec<Vec<families::Family>> = fns
        .iter()
        .map(|f| {
            let mut v = families(f);
            v.extend(own_families());
            v
        })
        .collect();
    let mut work = Vec::new();
    for (fi, fam) in fams.iter().enumerate() {
        for ki in 0..fam.len() {
            for c in 0..chunks {
                work.push((fi, ki, c));
            }
        }
    }
    let res = parallel(work.len(), threads, |w| {
        let (fi, ki, c) = work[w];
        let (f, fam) = (fns[fi], &fams[fi][ki]);
        let g = stream(f);
        // the record's families: the indices after the record's; this tool's: from 0
        let base = if fam.id <= 8 { COUNT } else { 0 };
        let mut o = Fresh::default();
        for i in base + c * chunk..base + (c + 1) * chunk {
            let x = (fam.arg)(random(g, fam.id, i));
            let a = ours(f, sse2, x);
            let b = m.eval(f, x);
            o.compared += 1;
            let mut bad = false;
            for k in 0..words(f) {
                if a[k] != b[k] {
                    o.differ[k] += 1;
                    bad = true;
                }
                if f64::from_bits(b[k]).is_nan() {
                    o.nan += 1;
                }
            }
            if bad && o.examples.len() < 8 {
                o.examples.push(format!(
                    "\"{f} family {} index {i}: x {:016x} larmorx {:016x} {:016x} glibc {:016x} {:016x}\"",
                    fam.id,
                    x.to_bits(),
                    a[0],
                    a[1],
                    b[0],
                    b[1]
                ));
            }
        }
        o
    });
    let mut ok = true;
    let mut js = String::new();
    let _ = write!(
        js,
        "{{\n  \"what\": \"larmorx_core::math's clean-room sin_glibc, cos_glibc, sincos_glibc{} against the system's functions on fresh arguments\",\n  \"glibc\": \"{}\",\n  \"cpu_fma_avx2\": {},\n  \"glibc_tunables\": \"{tunables}\",\n  \"larmorx_commit\": \"{}\",\n  \"arguments_per_family\": {per},\n  \"functions\": {{",
        if sse2 { " (sse2 builds)" } else { "" },
        libc_version(),
        is_x86_feature_detected!("fma") && is_x86_feature_detected!("avx2"),
        larmorx_commit()
    );
    let mut w = 0;
    for (fi, f) in fns.iter().enumerate() {
        let mut fam_js = Vec::new();
        let mut total = Fresh::default();
        for fam in &fams[fi] {
            let mut t = Fresh::default();
            for r in &res[w..w + chunks as usize] {
                t.compared += r.compared;
                t.differ[0] += r.differ[0];
                t.differ[1] += r.differ[1];
                t.nan += r.nan;
                t.examples.extend(r.examples.iter().cloned());
            }
            w += chunks as usize;
            t.examples.truncate(8);
            fam_js.push(format!(
                "\n        {{\"id\": {}, \"name\": \"{}\", \"compared\": {}, \"differ\": {}, \"glibc_nan_results\": {}, \"examples\": [{}]}}",
                fam.id,
                fam.name,
                t.compared,
                if words(f) == 2 {
                    format!("[{}, {}]", t.differ[0], t.differ[1])
                } else {
                    t.differ[0].to_string()
                },
                t.nan,
                t.examples.join(", ")
            ));
            total.compared += t.compared;
            total.differ[0] += t.differ[0];
            total.differ[1] += t.differ[1];
            total.nan += t.nan;
            if total.examples.len() < 8 {
                total.examples.extend(t.examples);
            }
        }
        let d = total.differ[0] + total.differ[1];
        ok &= d == 0;
        println!(
            "{f:7}{}: {} arguments in {} families, differ {:?}, glibc NaN results {}{}",
            if sse2 { " (sse2)" } else { "" },
            total.compared,
            fams[fi].len(),
            &total.differ[..words(f)],
            total.nan,
            if d == 0 { "" } else { "  DIFFER" }
        );
        for e in total.examples.iter().take(4) {
            println!("  {e}");
        }
        let _ = write!(
            js,
            "{}\n    \"{f}\": {{\"compared\": {}, \"differ\": {}, \"families\": [{}\n      ]}}",
            if fi > 0 { "," } else { "" },
            total.compared,
            d,
            fam_js.join(",")
        );
    }
    let secs = t0.elapsed().as_secs_f64();
    let _ = write!(
        js,
        "\n  }},\n  \"result\": \"{}\",\n  \"seconds\": {secs:.1},\n  \"threads\": {threads}\n}}\n",
        if ok { "PASS" } else { "FAIL" }
    );
    println!(
        "{} ({secs:.1} s on {threads} threads, glibc {})",
        if ok { "FRESH-PASS" } else { "FRESH-FAIL" },
        libc_version()
    );
    if let Some(p) = out {
        std::fs::write(&p, js).expect("write the result");
    }
    ok
}

// ---- hashes for the unit tests ----

/// The unit tests' arguments (`larmorx_core::math::sincos_glibc`'s tests, the same code): for
/// `k = 0 … 65535`, SplitMix64 of `k` mapped by `k mod 4` to a bit pattern, a uniform value on
/// [-8, 8), a uniform value on [-2^28, 2^28), a log-uniform magnitude 2^-30 to 2^33.
fn unit_test_arg(k: u64) -> f64 {
    let r = splitmix64(k);
    let u = (r >> 11) as f64 * f64::from_bits(0x3ca0_0000_0000_0000);
    match k % 4 {
        0 => f64::from_bits(r),
        1 => u * 16.0 - 8.0,
        2 => u * 536_870_912.0 - 268_435_456.0,
        _ => {
            let e = ((r >> 52) & 0x7ff) % 64;
            f64::from_bits(
                (r & 0x8000_0000_0000_0000) | ((993 + e) << 52) | (r & 0x000f_ffff_ffff_ffff),
            )
        }
    }
}

fn hashes(sse2: bool) {
    let m = Libm::open();
    let mut h = [FNV0; 4];
    let mut mine = [FNV0; 4];
    for k in 0..65_536u64 {
        let x = unit_test_arg(k);
        let (s, c) = (m.eval("sin", x)[0], m.eval("cos", x)[0]);
        let sc = m.eval("sincos", x);
        let o = [ours("sin", sse2, x)[0], ours("cos", sse2, x)[0]];
        let osc = ours("sincos", sse2, x);
        h[0] = fnv(h[0], s);
        h[1] = fnv(h[1], c);
        h[2] = fnv(h[2], sc[0]);
        h[3] = fnv(h[3], sc[1]);
        mine[0] = fnv(mine[0], o[0]);
        mine[1] = fnv(mine[1], o[1]);
        mine[2] = fnv(mine[2], osc[0]);
        mine[3] = fnv(mine[3], osc[1]);
    }
    println!(
        "glibc {} (GLIBC_TUNABLES={:?}), the unit tests' 65,536 arguments{}:",
        libc_version(),
        std::env::var("GLIBC_TUNABLES").unwrap_or_default(),
        if sse2 { ", sse2 builds" } else { "" }
    );
    for (i, n) in ["sin", "cos", "sincos (sine)", "sincos (cosine)"]
        .iter()
        .enumerate()
    {
        println!(
            "  0x{:04x}_{:04x}_{:04x}_{:04x}, // {n}{}",
            h[i] >> 48,
            (h[i] >> 32) & 0xffff,
            (h[i] >> 16) & 0xffff,
            h[i] & 0xffff,
            if h[i] == mine[i] {
                ""
            } else {
                "  (larmorx differs)"
            }
        );
    }
}

// ---- speed ----

/// A range of arguments for `bench`: its name, the argument made from a random number.
type Range = (&'static str, Box<dyn Fn(u64) -> f64>);

fn bench(log2n: u32) {
    let m = Libm::open();
    let n = 1usize << log2n;
    let ranges: [Range; 3] = [
        (
            "uniform on [-pi, pi)",
            Box::new(|r| {
                ((r >> 11) as f64 * f64::from_bits(0x3ca0_0000_0000_0000) - 0.5)
                    * std::f64::consts::TAU
            }),
        ),
        (
            "uniform on [-1e5, 1e5)",
            Box::new(|r| ((r >> 11) as f64 * f64::from_bits(0x3ca0_0000_0000_0000) - 0.5) * 2e5),
        ),
        (
            "log-uniform 2^27 to 2^1023",
            Box::new(|r| {
                let e = 27 + ((r >> 52) & 0x7ff) % 996;
                f64::from_bits(((1023 + e) << 52) | (r & 0x000f_ffff_ffff_ffff))
            }),
        ),
    ];
    println!("ns per call, one thread, {n} arguments per range (best of 5)");
    for (name, make) in &ranges {
        let xs: Vec<f64> = (0..n as u64)
            .map(|i| make(splitmix64(i ^ 0xbe7c_0000_0000_0000)))
            .collect();
        let time = |f: &dyn Fn(f64) -> f64| {
            let mut best = f64::INFINITY;
            for _ in 0..5 {
                let t = Instant::now();
                let mut acc = 0.0f64;
                for &x in &xs {
                    acc += f(black_box(x));
                }
                black_box(acc);
                best = best.min(t.elapsed().as_secs_f64() * 1e9 / n as f64);
            }
            best
        };
        let gsincos = |x: f64| {
            let (mut s, mut c) = (0.0, 0.0);
            (m.sincos)(x, &mut s, &mut c);
            s + c
        };
        let rows = [
            ("sin_glibc", time(&|x| lx::sin_glibc(x))),
            ("cos_glibc", time(&|x| lx::cos_glibc(x))),
            (
                "sincos_glibc",
                time(&|x| {
                    let (s, c) = lx::sincos_glibc(x);
                    s + c
                }),
            ),
            ("sin (CORE-MATH)", time(&|x| lx::sin(x))),
            ("cos (CORE-MATH)", time(&|x| lx::cos(x))),
            ("glibc sin", time(&|x| (m.sin)(x))),
            ("glibc cos", time(&|x| (m.cos)(x))),
            ("glibc sincos", time(&gsincos)),
        ];
        let line: Vec<String> = rows.iter().map(|(k, v)| format!("{k} {v:.1}")).collect();
        println!("{name}: {}", line.join("; "));
    }
}

fn list(f: &str, family: u64, first: u64, count: u64, sse2: bool) {
    let mut fams = families(f);
    fams.extend(own_families());
    let fam = fams
        .iter()
        .find(|x| x.id == family)
        .expect("unknown family");
    let g = stream(f);
    for i in first..first + count {
        let x = (fam.arg)(random(g, fam.id, i));
        let r = ours(f, sse2, x);
        if words(f) == 2 {
            println!("{i} {:016x} {:016x} {:016x}", x.to_bits(), r[0], r[1]);
        } else {
            println!("{i} {:016x} {:016x}", x.to_bits(), r[0]);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let get = |k: &str| {
        args.iter()
            .position(|a| a == k)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let flag = |k: &str| args.iter().any(|a| a == k);
    let threads = get("--threads").map_or(8, |v| v.parse().expect("--threads N"));
    let ok = match args.first().map(String::as_str) {
        Some("record") => record(
            &get("--record").expect("--record <json>"),
            flag("--sse2"),
            threads,
            get("--out"),
        ),
        Some("fresh") => fresh(
            flag("--sse2"),
            get("--log2n").map_or(27, |v| v.parse().expect("--log2n K")),
            threads,
            get("--out"),
        ),
        Some("hashes") => {
            hashes(flag("--sse2"));
            true
        }
        Some("bench") => {
            bench(get("--log2n").map_or(22, |v| v.parse().expect("--log2n K")));
            true
        }
        Some("list") if args.len() >= 5 => {
            list(
                &args[1],
                args[2].parse().expect("family"),
                args[3].parse().expect("first"),
                args[4].parse().expect("count"),
                flag("--sse2"),
            );
            true
        }
        _ => {
            eprintln!(
                "usage: cleanroom-glibc-dbl-verify record --record <json> [--sse2] [--threads N] [--out f]\n       \
                 cleanroom-glibc-dbl-verify fresh [--sse2] [--log2n K] [--threads N] [--out f]\n       \
                 cleanroom-glibc-dbl-verify hashes [--sse2] | bench [--log2n K]\n       \
                 cleanroom-glibc-dbl-verify list <function> <family> <first> <count> [--sse2]"
            );
            std::process::exit(2);
        }
    };
    std::process::exit(if ok { 0 } else { 1 });
}
