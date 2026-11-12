// SPDX-License-Identifier: Apache-2.0
//! `larmorx_core::x87::F80` against the machine's x87.
//!
//! `x87-probe <libx87probe.so> [log2n] [--record <file>]`: for every operation of
//! `larmorx_core::x87::cases::Op` and every operand family of `cases::Family`, `2^log2n` seeded
//! cases (default 20); every pair of `cases::specials()`; every conversion of `cases::Conv`
//! (`2^log2n` inputs) and every arithmetic pattern of `cases::Pattern` (`2^(log2n - 4)` cases),
//! computed by the software and by the hardware (`x87_ops.c`, `patterns.cpp`, compiled by GCC
//! 8.4.0 with FreeSurfer's flags: `build.sh`), compared bit for bit (NaNs included). Prints the
//! counts and `VERIFY-PASS` or `VERIFY-FAIL` (exit status 1). `--record` writes the hashes of the
//! hardware's first results (the cases the unit tests of `larmorx_core::x87` run) as Rust
//! constants. Meaningful on x86-64 Linux (`long double` is the x87's format there).

use std::ffi::{CString, c_char, c_int, c_void};
use std::fmt::Write as _;

use larmorx_core::x87::F80;
use larmorx_core::x87::cases::{self, Conv, Family, Op, Pattern, SplitMix64};

/// Cases per (operation, family) the unit tests run and the record hashes.
const TEST_N: usize = 1024;
/// Pattern cases the unit tests run.
const TEST_PATTERNS: usize = 256;
const CHUNK: usize = 1 << 16;

unsafe extern "C" {
    fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

type Bin = unsafe extern "C" fn(*const u8, *const u8, *mut u8, i64);
type Un = unsafe extern "C" fn(*const u8, *mut u8, i64);
type Cw = unsafe extern "C" fn() -> u16;
type PDot = unsafe extern "C" fn(*const f64, *const f64, i32) -> f64;
type PStats = unsafe extern "C" fn(*const f64, i32, *mut f64);
type PCross = unsafe extern "C" fn(*const f32, *mut f64);
type PUnit = unsafe extern "C" fn(*const f64, *mut f64);
type PSmooth = unsafe extern "C" fn(*const f32, i32, *mut f32, *mut f64);
type PEdge = unsafe extern "C" fn(*const f64, i32, *mut f64);

struct Lib(*mut c_void);

impl Lib {
    fn open(path: &str) -> Lib {
        let c = CString::new(path).unwrap();
        // SAFETY: the dynamic loader with a NUL-terminated name (RTLD_NOW).
        let h = unsafe { dlopen(c.as_ptr(), 2) };
        assert!(!h.is_null(), "dlopen({path})");
        Lib(h)
    }

    fn sym(&self, name: &str) -> *mut c_void {
        let c = CString::new(name).unwrap();
        // SAFETY: a symbol lookup in the library opened above.
        let f = unsafe { dlsym(self.0, c.as_ptr()) };
        assert!(!f.is_null(), "dlsym({name})");
        f
    }
}

macro_rules! sym {
    ($lib:expr, $name:expr, $t:ty) => {
        // SAFETY: the symbol is the C function of x87_ops.c or patterns.cpp with this signature.
        unsafe { std::mem::transmute::<*mut c_void, $t>($lib.sym($name)) }
    };
}

/// An array of `long double` slots (16 bytes each, the value in the first ten).
fn slots(v: &[F80]) -> Vec<[u8; 16]> {
    v.iter()
        .map(|x| {
            let mut s = [0u8; 16];
            s[..10].copy_from_slice(&x.to_le_bytes());
            s
        })
        .collect()
}

fn from_slot(s: &[u8]) -> F80 {
    F80::from_le_bytes(s[..10].try_into().unwrap())
}

/// The hardware's version of an operation on `a`, `b` (`b` ignored by the one-operand ones),
/// as `Op::apply` words.
struct Hw {
    add: Bin,
    sub: Bin,
    mul: Bin,
    div: Bin,
    cmp: Bin,
    sqrt: [(&'static str, Un); 3],
    to: [(Op, Un, usize); 8],
    from: [(Conv, Un, usize); 6],
}

impl Hw {
    fn load(lib: &Lib) -> Hw {
        Hw {
            add: sym!(lib, "x87_add", Bin),
            sub: sym!(lib, "x87_sub", Bin),
            mul: sym!(lib, "x87_mul", Bin),
            div: sym!(lib, "x87_div", Bin),
            cmp: sym!(lib, "x87_cmp", Bin),
            sqrt: [
                ("fsqrt", sym!(lib, "x87_sqrt_insn", Un)),
                ("sqrtl", sym!(lib, "x87_sqrtl", Un)),
                ("__builtin_sqrtl", sym!(lib, "x87_builtin_sqrt", Un)),
            ],
            to: [
                (Op::ToF64, sym!(lib, "x87_to_f64", Un), 8),
                (Op::ToF32, sym!(lib, "x87_to_f32", Un), 4),
                (Op::ToI64, sym!(lib, "x87_to_i64", Un), 8),
                (Op::ToI32, sym!(lib, "x87_to_i32", Un), 4),
                (Op::ToI16, sym!(lib, "x87_to_i16", Un), 2),
                (Op::ToU64, sym!(lib, "x87_to_u64", Un), 8),
                (Op::ToU32, sym!(lib, "x87_to_u32", Un), 4),
                (Op::RoundToI64, sym!(lib, "x87_llrintl", Un), 8),
            ],
            from: [
                (Conv::F64, sym!(lib, "x87_from_f64", Un), 8),
                (Conv::F32, sym!(lib, "x87_from_f32", Un), 4),
                (Conv::I64, sym!(lib, "x87_from_i64", Un), 8),
                (Conv::I32, sym!(lib, "x87_from_i32", Un), 4),
                (Conv::U64, sym!(lib, "x87_from_u64", Un), 8),
                (Conv::U32, sym!(lib, "x87_from_u32", Un), 4),
            ],
        }
    }

    /// The hardware's results of `op` (one variant per sqrt implementation).
    fn run(&self, op: Op, a: &[F80], b: &[F80]) -> Vec<(&'static str, Vec<u128>)> {
        let n = a.len();
        let (sa, sb) = (slots(a), slots(b));
        let pa = sa.as_ptr() as *const u8;
        let pb = sb.as_ptr() as *const u8;
        let ext = |f: Bin| {
            let mut r = vec![[0u8; 16]; n];
            // SAFETY: n slots in, n slots out.
            unsafe { f(pa, pb, r.as_mut_ptr() as *mut u8, n as i64) };
            r.iter().map(|s| from_slot(s).to_bits()).collect::<Vec<_>>()
        };
        match op {
            Op::Add => vec![("", ext(self.add))],
            Op::Sub => vec![("", ext(self.sub))],
            Op::Mul => vec![("", ext(self.mul))],
            Op::Div => vec![("", ext(self.div))],
            Op::Cmp => {
                let mut r = vec![0u8; n];
                // SAFETY: n slots in, n bytes out.
                unsafe { (self.cmp)(pa, pb, r.as_mut_ptr(), n as i64) };
                vec![("", r.into_iter().map(u128::from).collect())]
            }
            Op::Sqrt => self
                .sqrt
                .iter()
                .map(|&(name, f)| {
                    let mut r = vec![[0u8; 16]; n];
                    // SAFETY: n slots in, n slots out.
                    unsafe { f(pa, r.as_mut_ptr() as *mut u8, n as i64) };
                    (name, r.iter().map(|s| from_slot(s).to_bits()).collect())
                })
                .collect(),
            _ => {
                let &(_, f, size) = self.to.iter().find(|t| t.0 == op).unwrap();
                let mut r = vec![0u8; n * size];
                // SAFETY: n slots in, n values of `size` bytes out.
                unsafe { f(pa, r.as_mut_ptr(), n as i64) };
                vec![(
                    "",
                    r.chunks(size)
                        .map(|c| {
                            let mut w = [0u8; 16];
                            w[..size].copy_from_slice(c);
                            u128::from_le_bytes(w)
                        })
                        .collect(),
                )]
            }
        }
    }

    fn run_from(&self, conv: Conv, x: &[u64]) -> Vec<u128> {
        let &(_, f, size) = self.from.iter().find(|t| t.0 == conv).unwrap();
        let input: Vec<u8> = x.iter().flat_map(|w| w.to_le_bytes()[..size].to_vec()).collect();
        let mut r = vec![[0u8; 16]; x.len()];
        // SAFETY: n values of `size` bytes in, n slots out.
        unsafe { f(input.as_ptr(), r.as_mut_ptr() as *mut u8, x.len() as i64) };
        r.iter().map(|s| from_slot(s).to_bits()).collect()
    }
}

struct Patterns {
    dot: PDot,
    stats: PStats,
    cross: PCross,
    unit: PUnit,
    smooth: PSmooth,
    edge: PEdge,
}

impl Patterns {
    fn load(lib: &Lib) -> Patterns {
        Patterns {
            dot: sym!(lib, "pat_dot", PDot),
            stats: sym!(lib, "pat_stats", PStats),
            cross: sym!(lib, "pat_cross", PCross),
            unit: sym!(lib, "pat_unit", PUnit),
            smooth: sym!(lib, "pat_smooth", PSmooth),
            edge: sym!(lib, "pat_edgecost", PEdge),
        }
    }

    fn run(&self, p: Pattern, x: &cases::PatternInput) -> Vec<u64> {
        let n = x.n;
        let d = x.doubles.as_ptr();
        let f = x.floats.as_ptr();
        let bits = |v: Vec<f64>| v.into_iter().map(f64::to_bits).collect::<Vec<u64>>();
        // SAFETY: each call reads and writes the counts its C++ function documents.
        unsafe {
            match p {
                Pattern::Dot => vec![(self.dot)(d, d.add(n), n as i32).to_bits()],
                Pattern::Stats => {
                    let mut o = vec![0.0; 3];
                    (self.stats)(d, n as i32, o.as_mut_ptr());
                    bits(o)
                }
                Pattern::Cross => {
                    let mut o = vec![0.0; 4];
                    (self.cross)(f, o.as_mut_ptr());
                    bits(o)
                }
                Pattern::Unit => {
                    let mut o = vec![0.0; 13];
                    (self.unit)(d, o.as_mut_ptr());
                    bits(o)
                }
                Pattern::Smooth => {
                    let mut o = vec![0f32; n];
                    let mut o2 = vec![0.0; 2];
                    (self.smooth)(f, n as i32, o.as_mut_ptr(), o2.as_mut_ptr());
                    let mut w: Vec<u64> = o.into_iter().map(|v| u64::from(v.to_bits())).collect();
                    w.extend(bits(o2));
                    w
                }
                Pattern::EdgeCost => {
                    let mut o = vec![0.0; n + 1];
                    (self.edge)(d, n as i32, o.as_mut_ptr());
                    bits(o)
                }
            }
        }
    }
}

/// The kinds of the hardware's extended results (coverage evidence).
#[derive(Default)]
struct Classes {
    nan: u64,
    indefinite: u64,
    inf: u64,
    zero: u64,
    denormal: u64,
    normal: u64,
    ties: u64,
}

impl Classes {
    fn add(&mut self, bits: u128, tie: bool) {
        let e = (bits >> 64) as u16 & 0x7fff;
        let m = bits as u64;
        if bits == F80::INDEFINITE.to_bits() {
            self.indefinite += 1;
        } else if e == 0x7fff && m == 1 << 63 {
            self.inf += 1;
        } else if e == 0x7fff {
            self.nan += 1;
        } else if e == 0 && m == 0 {
            self.zero += 1;
        } else if e == 0 {
            self.denormal += 1;
        } else {
            self.normal += 1;
        }
        self.ties += u64::from(tie);
    }

    fn summary(&self) -> String {
        let all = self.nan + self.indefinite + self.inf + self.zero + self.denormal + self.normal;
        if all == 0 {
            return String::new();
        }
        let mut s = format!(
            " (results: {} normal, {} denormal, {} zero, {} infinite, {} indefinite, {} other NaN",
            self.normal, self.denormal, self.zero, self.inf, self.indefinite, self.nan
        );
        if self.ties > 0 {
            write!(s, "; {} exact ties", self.ties).unwrap();
        }
        s.push(')');
        s
    }
}

/// Whether the exact product of two normal numbers lies exactly halfway between two extended
/// numbers (a tie for rounding to nearest), ignoring the range.
fn tie_product(a: F80, b: F80) -> bool {
    let normal = |x: F80| {
        let e = (x.to_bits() >> 64) as u16 & 0x7fff;
        e != 0 && e != 0x7fff && (x.to_bits() as u64) >> 63 == 1
    };
    if !normal(a) || !normal(b) {
        return false;
    }
    let p = u128::from(a.to_bits() as u64) * u128::from(b.to_bits() as u64);
    let p = p << p.leading_zeros();
    p as u64 == 1 << 63
}

#[derive(Default)]
struct Tally {
    cases: u64,
    differ: u64,
    shown: u32,
}

impl Tally {
    fn check(&mut self, what: &str, inputs: &dyn Fn() -> String, sw: u128, hw: u128) {
        self.cases += 1;
        if sw != hw {
            self.differ += 1;
            if self.shown < 5 {
                self.shown += 1;
                println!("  DIFF {what}: {} software {sw:#x} hardware {hw:#x}", inputs());
            }
        }
    }
}

/// A pattern computed in `double` instead of `long double`: the control that shows the patterns
/// depend on the extended precision.
fn in_double(p: Pattern, x: &cases::PatternInput) -> Vec<u64> {
    let n = x.n;
    let d = &x.doubles;
    let f = &x.floats;
    let b = |v: f64| v.to_bits();
    match p {
        Pattern::Dot => vec![b((0..n).fold(0.0, |s, i| s + d[i] * d[n + i]))],
        Pattern::Stats => {
            let (mut s, mut s2) = (0.0, 0.0);
            for &v in &d[..n] {
                s += v;
                s2 += v * v;
            }
            let mean = s / n as f64;
            let var = (s2 - n as f64 * mean * mean) / (n as f64 - 1.0);
            vec![b(mean), b(var), b(var.sqrt())]
        }
        Pattern::Cross => {
            let g = |i: usize| f64::from(f[i]);
            let a = [g(3) - g(0), g(4) - g(1), g(5) - g(2)];
            let c = [g(6) - g(3), g(7) - g(4), g(8) - g(5)];
            let cx = a[1] * c[2] - a[2] * c[1];
            let cy = a[2] * c[0] - a[0] * c[2];
            let cz = a[0] * c[1] - a[1] * c[0];
            vec![b(cx), b(cy), b(cz), b((cx * cx + cy * cy + cz * cz).sqrt())]
        }
        Pattern::Unit => {
            let w = [d[3] - d[0], d[4] - d[1], d[5] - d[2]];
            let l = (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt();
            let mut o = vec![b(l), b(w[0] / l), b(w[1] / l), b(w[2] / l)];
            let (il3, il) = (1.0 / (l * l * l), 1.0 / l);
            for k in 0..3 {
                for m in 0..3 {
                    let mut a = (w[k] * w[m]) * il3;
                    if k == m {
                        a -= il;
                    }
                    o.push(b(a));
                }
            }
            o
        }
        Pattern::Smooth => {
            let (mut s, mut v2) = (0.0, 0.0);
            for &v in &f[..n] {
                s += f64::from(v);
                v2 += f64::from(v * v);
            }
            let mut o: Vec<u64> = f[..n]
                .iter()
                .map(|&v| u64::from(((f64::from(v) / s) as f32).to_bits()))
                .collect();
            o.push(b(v2));
            o.push(b(1.0 / v2));
            o
        }
        Pattern::EdgeCost => {
            let l0 = d[0];
            let mut t = 0.0;
            let mut o = Vec::new();
            for &len in &d[1..=n] {
                let dl = len - l0;
                t += dl * dl;
                o.push(b(2.0 * dl / len));
            }
            o.push(b(t));
            o
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let lib_path = args.get(1).expect("usage: x87-probe <libx87probe.so> [log2n] [--record <file>]");
    let mut log2n = 20u32;
    let mut record: Option<String> = None;
    let mut i = 2;
    while i < args.len() {
        if args[i] == "--record" {
            record = Some(args[i + 1].clone());
            i += 2;
        } else {
            log2n = args[i].parse().expect("log2n");
            i += 1;
        }
    }
    let n_cases = 1usize << log2n;
    let lib = Lib::open(lib_path);
    let hw = Hw::load(&lib);
    let pats = Patterns::load(&lib);
    let control_word = sym!(lib, "x87_control_word", Cw);
    // SAFETY: x87_control_word() takes nothing and returns the control word.
    let cw = unsafe { control_word() };
    println!("x87 control word {cw:#06x} (Linux's default 0x037f: 64-bit precision, nearest, masked)");
    assert_eq!(cw, 0x037f, "the probe needs Linux's default control word");

    let mut total = Tally::default();
    let mut rec = String::new();
    writeln!(rec, "// Recorded by validation/probes/x87-extended (x87-probe --record) from the machine's x87.").unwrap();
    writeln!(rec, "// The hardware's results of the first {TEST_N} cases of each (operation, family).").unwrap();
    writeln!(rec, "pub(super) const OP_FAMILY_HASHES: [[u64; {}]; {}] = [", Family::ALL.len(), Op::ALL.len()).unwrap();

    for (o, &op) in Op::ALL.iter().enumerate() {
        let mut row = Vec::new();
        for (f, &fam) in Family::ALL.iter().enumerate() {
            let mut rng = SplitMix64::new(cases::seed(o, f));
            let mut t = Tally::default();
            let mut first: Vec<u128> = Vec::new();
            let mut classes = Classes::default();
            let mut done = 0;
            while done < n_cases {
                let m = CHUNK.min(n_cases - done);
                let (a, b): (Vec<F80>, Vec<F80>) = (0..m).map(|_| fam.pair(&mut rng)).unzip();
                let sw: Vec<u128> = a.iter().zip(&b).map(|(&x, &y)| op.apply(x, y)).collect();
                for (name, h) in hw.run(op, &a, &b) {
                    if matches!(op, Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Sqrt)
                        && (name.is_empty() || name == "fsqrt")
                    {
                        for k in 0..m {
                            classes.add(h[k], op == Op::Mul && tie_product(a[k], b[k]));
                        }
                    }
                    for k in 0..m {
                        let inputs = || format!("a={:#x} b={:#x}", a[k].to_bits(), b[k].to_bits());
                        t.check(&format!("{op:?}{name} {fam:?}"), &inputs, sw[k], h[k]);
                    }
                    if done == 0 && first.is_empty() {
                        first = h[..TEST_N.min(m)].to_vec();
                    }
                }
                done += m;
            }
            println!("{op:?} {fam:?}: {} cases, {} differ{}", t.cases, t.differ, classes.summary());
            total.cases += t.cases;
            total.differ += t.differ;
            row.push(cases::fnv(first));
        }
        writeln!(rec, "    [{}], // {op:?}", row.iter().map(|h| format!("{h:#018x}")).collect::<Vec<_>>().join(", ")).unwrap();
    }
    writeln!(rec, "];").unwrap();

    // Every pair of special values.
    let s = cases::specials();
    let (a, b): (Vec<F80>, Vec<F80>) = s.iter().flat_map(|&x| s.iter().map(move |&y| (x, y))).unzip();
    writeln!(rec, "// Every pair of cases::specials(), in order, per operation.").unwrap();
    writeln!(rec, "pub(super) const SPECIAL_HASHES: [u64; {}] = [", Op::ALL.len()).unwrap();
    for &op in &Op::ALL {
        let sw: Vec<u128> = a.iter().zip(&b).map(|(&x, &y)| op.apply(x, y)).collect();
        let mut t = Tally::default();
        let mut hash = 0;
        for (name, h) in hw.run(op, &a, &b) {
            for k in 0..a.len() {
                let inputs = || format!("a={:#x} b={:#x}", a[k].to_bits(), b[k].to_bits());
                t.check(&format!("{op:?}{name} specials"), &inputs, sw[k], h[k]);
            }
            if hash == 0 {
                hash = cases::fnv(h);
            }
        }
        println!("{op:?} specials: {} cases, {} differ", t.cases, t.differ);
        total.cases += t.cases;
        total.differ += t.differ;
        writeln!(rec, "    {hash:#018x}, // {op:?}").unwrap();
    }
    writeln!(rec, "];").unwrap();

    // Conversions into the extended format.
    writeln!(rec, "// The first {TEST_N} inputs of each conversion into the extended format.").unwrap();
    writeln!(rec, "pub(super) const CONV_HASHES: [u64; {}] = [", Conv::ALL.len()).unwrap();
    for (c, &conv) in Conv::ALL.iter().enumerate() {
        let mut rng = SplitMix64::new(cases::from_seed(c));
        let x: Vec<u64> = (0..n_cases).map(|_| conv.input(&mut rng)).collect();
        let h = hw.run_from(conv, &x);
        let mut t = Tally::default();
        for k in 0..x.len() {
            let inputs = || format!("x={:#x}", x[k]);
            t.check(&format!("{conv:?}"), &inputs, conv.apply(x[k]), h[k]);
        }
        println!("from {conv:?}: {} cases, {} differ", t.cases, t.differ);
        total.cases += t.cases;
        total.differ += t.differ;
        writeln!(rec, "    {:#018x}, // {conv:?}", cases::fnv(h[..TEST_N].iter().copied())).unwrap();
    }
    writeln!(rec, "];").unwrap();

    // The patterns.
    let n_patterns = (n_cases >> 4).max(TEST_PATTERNS);
    writeln!(rec, "// The outputs of the first {TEST_PATTERNS} cases of each pattern.").unwrap();
    writeln!(rec, "pub(super) const PATTERN_HASHES: [u64; {}] = [", Pattern::ALL.len()).unwrap();
    for (p, &pat) in Pattern::ALL.iter().enumerate() {
        let mut rng = SplitMix64::new(cases::pattern_seed(p));
        let mut t = Tally::default();
        let mut words: Vec<u128> = Vec::new();
        let mut control = 0usize;
        for k in 0..n_patterns {
            let x = pat.inputs(&mut rng);
            let sw = pat.apply(&x);
            let h = pats.run(pat, &x);
            assert_eq!(sw.len(), h.len());
            control += in_double(pat, &x).iter().zip(&h).filter(|(c, w)| c != w).count();
            for (j, (&s, &w)) in sw.iter().zip(&h).enumerate() {
                let inputs = || format!("case {k} output {j} {x:?}");
                t.check(&format!("{pat:?}"), &inputs, u128::from(s), u128::from(w));
            }
            if k < TEST_PATTERNS {
                words.extend(h.iter().map(|&w| u128::from(w)));
            }
        }
        println!(
            "pattern {pat:?}: {} outputs of {n_patterns} cases, {} differ (control: {control} of them differ when computed in double)",
            t.cases, t.differ
        );
        total.cases += t.cases;
        total.differ += t.differ;
        writeln!(rec, "    {:#018x}, // {pat:?}", cases::fnv(words)).unwrap();
    }
    writeln!(rec, "];").unwrap();

    if let Some(path) = record {
        std::fs::write(&path, rec).expect("write the record");
        println!("hashes written to {path}");
    }
    println!("total: {} comparisons, {} differ", total.cases, total.differ);
    if total.differ == 0 {
        println!("VERIFY-PASS");
    } else {
        println!("VERIFY-FAIL");
        std::process::exit(1);
    }
}
