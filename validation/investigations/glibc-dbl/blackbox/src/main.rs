// SPDX-License-Identifier: Apache-2.0
//! The black-box record of `specs/glibc-dbl.md`: glibc 2.35's double `sin`, `cos`, `sincos`,
//! `tan`, `asin`, `acos` and `atan`, called from the C library's `libm.so.6` (opened with
//! `dlopen`; on x86-64 with FMA and AVX2 glibc runs its FMA builds of `sin`, `cos`, `tan`,
//! `asin`, `acos` and `atan`, and its single build of `sincos`). glibc's code is never read here.
//!
//! - `record [--threads N] [--out record.json] [--only f,g]`: every family of `../families.rs`
//!   (2^27 arguments each), hashed in blocks of 2^22 results; per family, the hash of its block
//!   hashes and how many results differ from the correctly rounded value
//!   (`larmorx_core::math`'s CORE-MATH ports; `atan(x)` as `atan2(x, 1)`); and the result of
//!   every special argument.
//! - `list <function> <family> <first> <count>`: the arguments and glibc's results, one per
//!   line (`index argument-bits result-bits`; `sincos` gives the sine's bits, then the cosine's),
//!   to find, input by input, where an implementation differs inside a block.
//!
//! The hash (the cbrtf record's, on 64-bit words): start `h = 14695981039346656037`; for each
//! result, `h = (h XOR bits) * 1099511628211 mod 2^64`, the results in argument order, every
//! bit of the result as it is (NaNs included, no canonical NaN); `sincos` hashes the sine's
//! bits, then the cosine's, of each argument.

#[path = "../../families.rs"]
mod families;

use std::ffi::{CStr, c_char, c_int, c_void};
use std::fmt::Write as _;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use families::{BLOCK, COUNT, FUNCTIONS, families, random, special};
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
    tan: F64,
    asin: F64,
    acos: F64,
    atan: F64,
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
        // SAFETY: plain calls of the C library's dynamic loader; every symbol is cast to its C
        // signature (`double sin(double)`, `void sincos(double, double *, double *)`).
        unsafe {
            let h = dlopen(c"libm.so.6".as_ptr(), 2);
            if h.is_null() {
                panic!("dlopen(libm.so.6): {:?}", CStr::from_ptr(dlerror()));
            }
            use std::mem::transmute as t;
            Libm {
                sin: t::<*mut c_void, F64>(symbol(h, c"sin")),
                cos: t::<*mut c_void, F64>(symbol(h, c"cos")),
                sincos: t::<*mut c_void, SinCos>(symbol(h, c"sincos")),
                tan: t::<*mut c_void, F64>(symbol(h, c"tan")),
                asin: t::<*mut c_void, F64>(symbol(h, c"asin")),
                acos: t::<*mut c_void, F64>(symbol(h, c"acos")),
                atan: t::<*mut c_void, F64>(symbol(h, c"atan")),
            }
        }
    }

    /// glibc's result bits for `x` (one word; `sincos` two) and the correctly rounded ones.
    fn eval(&self, f: &str, x: f64) -> ([u64; 2], [u64; 2], usize) {
        let one = |g: f64, c: f64| ([g.to_bits(), 0], [c.to_bits(), 0], 1);
        match f {
            "sin" => one((self.sin)(x), lx::sin(x)),
            "cos" => one((self.cos)(x), lx::cos(x)),
            "tan" => one((self.tan)(x), lx::tan(x)),
            "asin" => one((self.asin)(x), lx::asin(x)),
            "acos" => one((self.acos)(x), lx::acos(x)),
            "atan" => one((self.atan)(x), lx::atan2(x, 1.0)),
            "sincos" => {
                let (mut s, mut c) = (0.0f64, 0.0f64);
                (self.sincos)(x, &mut s, &mut c);
                (
                    [s.to_bits(), c.to_bits()],
                    [lx::sin(x).to_bits(), lx::cos(x).to_bits()],
                    2,
                )
            }
            _ => unreachable!(),
        }
    }
}

const FNV0: u64 = 14_695_981_039_346_656_037;
const FNVP: u64 = 1_099_511_628_211;

#[inline]
fn fnv(h: u64, w: u64) -> u64 {
    (h ^ w).wrapping_mul(FNVP)
}

fn same(a: u64, b: u64) -> bool {
    a == b || (f64::from_bits(a).is_nan() && f64::from_bits(b).is_nan())
}

/// One block's outcome: its hash, how many results differ from the correctly rounded value (per
/// output word), how many NaN results.
#[derive(Clone, Copy, Default)]
struct Block {
    hash: u64,
    differ: [u64; 2],
    nan: u64,
}

fn run_block(m: &Libm, f: &str, g: u64, fam: &families::Family, b: u64) -> Block {
    let mut out = Block {
        hash: FNV0,
        ..Block::default()
    };
    for i in b * BLOCK..(b + 1) * BLOCK {
        let x = (fam.arg)(random(g, fam.id, i));
        let (r, c, n) = m.eval(f, x);
        for k in 0..n {
            out.hash = fnv(out.hash, r[k]);
            if !same(r[k], c[k]) {
                out.differ[k] += 1;
            }
            if f64::from_bits(r[k]).is_nan() {
                out.nan += 1;
            }
        }
    }
    out
}

fn libc_version() -> String {
    // SAFETY: glibc's function returns a static NUL-terminated string.
    unsafe { CStr::from_ptr(gnu_get_libc_version()) }
        .to_string_lossy()
        .into_owned()
}

fn record(threads: usize, out: Option<String>, only: Option<Vec<String>>) {
    let m = Libm::open();
    let t0 = Instant::now();
    let fns: Vec<(&str, u64)> = FUNCTIONS
        .iter()
        .copied()
        .filter(|(f, _)| only.as_ref().is_none_or(|o| o.iter().any(|x| x == f)))
        .collect();
    let blocks_per = COUNT / BLOCK;
    // the work: (function index, family index, block)
    let mut work = Vec::new();
    let fams: Vec<Vec<families::Family>> = fns.iter().map(|(f, _)| families(f)).collect();
    for (fi, fam) in fams.iter().enumerate() {
        for (ki, _) in fam.iter().enumerate() {
            for b in 0..blocks_per {
                work.push((fi, ki, b));
            }
        }
    }
    let results: Mutex<Vec<Option<Block>>> = Mutex::new(vec![None; work.len()]);
    let next = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let w = next.fetch_add(1, Ordering::Relaxed);
                    if w >= work.len() {
                        break;
                    }
                    let (fi, ki, b) = work[w];
                    let r = run_block(&m, fns[fi].0, fns[fi].1, &fams[fi][ki], b);
                    results.lock().unwrap()[w] = Some(r);
                }
            });
        }
    });
    let results: Vec<Block> = results
        .into_inner()
        .unwrap()
        .into_iter()
        .map(Option::unwrap)
        .collect();
    let fma = is_x86_feature_detected!("fma") && is_x86_feature_detected!("avx2");
    let mut js = String::new();
    let _ = write!(
        js,
        "{{\n  \"what\": \"glibc's double sin, cos, sincos, tan, asin, acos and atan as a black box (specs/glibc-dbl.md)\",\n  \
         \"glibc\": \"{}\",\n  \"cpu_fma_avx2\": {},\n  \"glibc_tunables\": \"{}\",\n  \"arguments_per_family\": {},\n  \
         \"block\": {},\n  \"hash\": \"FNV-1a on 64-bit words: h0 = 14695981039346656037, h = (h xor bits) * 1099511628211 mod 2^64; family hash = the same over its block hashes; sincos: sine then cosine\",\n  \
         \"functions\": {{",
        libc_version(),
        fma,
        std::env::var("GLIBC_TUNABLES").unwrap_or_default(),
        COUNT,
        BLOCK
    );
    let mut summary = String::new();
    let mut w = 0;
    for (fi, (f, _)) in fns.iter().enumerate() {
        let _ = write!(
            js,
            "{}\n    \"{f}\": {{\n      \"families\": [",
            if fi > 0 { "," } else { "" }
        );
        let words = if *f == "sincos" { 2 } else { 1 };
        let mut tot = [0u64; 2];
        for (ki, fam) in fams[fi].iter().enumerate() {
            let bl = &results[w..w + blocks_per as usize];
            w += blocks_per as usize;
            let fh = bl.iter().fold(FNV0, |h, b| fnv(h, b.hash));
            let d0: u64 = bl.iter().map(|b| b.differ[0]).sum();
            let d1: u64 = bl.iter().map(|b| b.differ[1]).sum();
            let nan: u64 = bl.iter().map(|b| b.nan).sum();
            tot[0] += d0;
            tot[1] += d1;
            let hashes: Vec<String> = bl.iter().map(|b| format!("\"{:016x}\"", b.hash)).collect();
            let differ = if words == 2 {
                format!("[{d0}, {d1}]")
            } else {
                format!("{d0}")
            };
            let _ = write!(
                js,
                "{}\n        {{\"id\": {}, \"name\": \"{}\", \"hash\": \"{fh:016x}\", \"differ_from_correctly_rounded\": {differ}, \"nan_results\": {nan},\n         \"blocks\": [{}]}}",
                if ki > 0 { "," } else { "" },
                fam.id,
                fam.name,
                hashes.join(", ")
            );
            let pct = |d: u64| 100.0 * d as f64 / COUNT as f64;
            if words == 2 {
                let _ = writeln!(
                    summary,
                    "{f:7} family {}: hash {fh:016x}; differ: sin {d0} ({:.4} %), cos {d1} ({:.4} %); NaN {nan}  [{}]",
                    fam.id,
                    pct(d0),
                    pct(d1),
                    fam.name
                );
            } else {
                let _ = writeln!(
                    summary,
                    "{f:7} family {}: hash {fh:016x}; differ {d0} ({:.4} %); NaN {nan}  [{}]",
                    fam.id,
                    pct(d0),
                    fam.name
                );
            }
        }
        // the special arguments, result by result
        let sp: Vec<String> = special()
            .into_iter()
            .map(|x| {
                let (r, _, n) = m.eval(f, x);
                if n == 2 {
                    format!(
                        "[\"{:016x}\", \"{:016x}\", \"{:016x}\"]",
                        x.to_bits(),
                        r[0],
                        r[1]
                    )
                } else {
                    format!("[\"{:016x}\", \"{:016x}\"]", x.to_bits(), r[0])
                }
            })
            .collect();
        let _ = write!(
            js,
            "\n      ],\n      \"special\": [{}]\n    }}",
            sp.join(", ")
        );
        let _ = writeln!(
            summary,
            "{f:7} all families: differ {:?} of {}",
            &tot[..words],
            COUNT * fams[fi].len() as u64
        );
    }
    js.push_str("\n  }\n}\n");
    print!("{summary}");
    println!(
        "glibc {}, FMA and AVX2: {fma}, {:.1} s on {threads} threads",
        libc_version(),
        t0.elapsed().as_secs_f64()
    );
    if let Some(p) = out {
        std::fs::write(&p, js).expect("write the record");
        println!("record written to {p}");
    }
}

fn list(f: &str, family: u64, first: u64, count: u64) {
    let m = Libm::open();
    let g = FUNCTIONS
        .iter()
        .find(|(n, _)| *n == f)
        .expect("unknown function")
        .1;
    let fams = families(f);
    let fam = fams
        .iter()
        .find(|x| x.id == family)
        .expect("unknown family");
    let mut s = String::new();
    for i in first..first + count {
        let x = (fam.arg)(random(g, fam.id, i));
        let (r, _, n) = m.eval(f, x);
        if n == 2 {
            let _ = writeln!(s, "{i} {:016x} {:016x} {:016x}", x.to_bits(), r[0], r[1]);
        } else {
            let _ = writeln!(s, "{i} {:016x} {:016x}", x.to_bits(), r[0]);
        }
    }
    print!("{s}");
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let get = |k: &str| {
        args.iter()
            .position(|a| a == k)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    match args.first().map(String::as_str) {
        Some("record") => record(
            get("--threads").map_or(8, |v| v.parse().expect("--threads N")),
            get("--out"),
            get("--only").map(|v| v.split(',').map(str::to_owned).collect()),
        ),
        Some("list") if args.len() == 5 => list(
            &args[1],
            args[2].parse().expect("family"),
            args[3].parse().expect("first"),
            args[4].parse().expect("count"),
        ),
        _ => {
            eprintln!(
                "usage: glibc-dbl-blackbox record [--threads N] [--out record.json] [--only f,g]\n       glibc-dbl-blackbox list <function> <family> <first> <count>"
            );
            std::process::exit(2);
        }
    }
}
