// SPDX-License-Identifier: Apache-2.0
//! `larmorx_core::vnl_random` and `larmorx_core::drand48` against their originals.
//!
//! `fs-random <libvnlrandom.so> [n_seeds] [--record <file>]`:
//! - **vnl_random**: ITK 4.13.2's own `vnl_random.cxx`, compiled by GCC 8.4.0 with FreeSurfer
//!   8.2.0's C++ flags (`build.sh`), against the port, on `n_seeds` seeds (default 2,000:
//!   FreeSurfer's 1234 and 53, 0, 1, `-1` as `unsigned long`, the rest random 64-bit values) and,
//!   per seed, 2,000 calls chosen at random among `lrand32`, `lrand32(a, b)`, `lrand32(a, b,
//!   count)`, `drand32(a, b)`, `drand64(a, b)`, `normal`, `normal64`, `restart`, `reseed` and
//!   FreeSurfer's `(float) drand64(0.0, 1.0)`; the logarithm of the normal numbers is
//!   `math::log_glibc` (the C library's, glibc 2.35 here), and how often the correctly rounded
//!   `math::log` would change a normal number is counted;
//! - **drand48**: the machine's C library (glibc 2.35, a black box: its code is never read)
//!   against the port: the state before any seeding, `srand48` with `n_seeds` seeds (53 first,
//!   `AntsN4BiasFieldCorrectionFs`'s), `seed48` with random words, `lcong48` with random
//!   parameters, each followed by 1,000 calls chosen at random among the nine functions (the
//!   array ones on a caller's array).
//!
//! Prints the counts and `VERIFY-PASS` or `VERIFY-FAIL` (exit status 1); `--record` writes the
//! hashes of the originals' sequences for a few seeds as Rust constants (the unit tests of
//! `larmorx_core::vnl_random` and `larmorx_core::drand48` check them on every platform).

use std::ffi::{CString, c_char, c_int, c_void};
use std::fmt::Write as _;

use larmorx_core::drand48::Drand48;
use larmorx_core::math;
use larmorx_core::vnl_random::{ARRAY_SIZE, VnlRandom};
use larmorx_core::x87::cases::SplitMix64;

unsafe extern "C" {
    fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    // The C library's generators (glibc), black boxes.
    fn drand48() -> f64;
    fn erand48(xsubi: *mut u16) -> f64;
    fn lrand48() -> i64;
    fn nrand48(xsubi: *mut u16) -> i64;
    fn mrand48() -> i64;
    fn jrand48(xsubi: *mut u16) -> i64;
    fn srand48(seedval: i64);
    fn seed48(seed16v: *mut u16) -> *mut u16;
    fn lcong48(param: *mut u16);
}

struct Vr {
    new: unsafe extern "C" fn(u64) -> *mut c_void,
    free: unsafe extern "C" fn(*mut c_void),
    reseed: unsafe extern "C" fn(*mut c_void, u64),
    reseed_array: unsafe extern "C" fn(*mut c_void, *mut u64),
    restart: unsafe extern "C" fn(*mut c_void),
    lrand32: unsafe extern "C" fn(*mut c_void) -> u64,
    range: unsafe extern "C" fn(*mut c_void, i32, i32) -> i32,
    range_count: unsafe extern "C" fn(*mut c_void, i32, i32, *mut i32) -> i32,
    drand32: unsafe extern "C" fn(*mut c_void, f64, f64) -> f64,
    drand64: unsafe extern "C" fn(*mut c_void, f64, f64) -> f64,
    normal: unsafe extern "C" fn(*mut c_void) -> f64,
    normal64: unsafe extern "C" fn(*mut c_void) -> f64,
    drand64_float: unsafe extern "C" fn(*mut c_void) -> f32,
}

fn load(path: &str) -> Vr {
    let c = CString::new(path).unwrap();
    // SAFETY: the dynamic loader with a NUL-terminated name (RTLD_NOW).
    let h = unsafe { dlopen(c.as_ptr(), 2) };
    assert!(!h.is_null(), "dlopen({path})");
    let sym = |name: &str| {
        let c = CString::new(name).unwrap();
        // SAFETY: a symbol lookup in the library opened above.
        let f = unsafe { dlsym(h, c.as_ptr()) };
        assert!(!f.is_null(), "dlsym({name})");
        f
    };
    // SAFETY: each symbol is the shim.cxx function with this signature.
    unsafe {
        Vr {
            new: std::mem::transmute::<*mut c_void, _>(sym("vr_new")),
            free: std::mem::transmute::<*mut c_void, _>(sym("vr_free")),
            reseed: std::mem::transmute::<*mut c_void, _>(sym("vr_reseed")),
            reseed_array: std::mem::transmute::<*mut c_void, _>(sym("vr_reseed_array")),
            restart: std::mem::transmute::<*mut c_void, _>(sym("vr_restart")),
            lrand32: std::mem::transmute::<*mut c_void, _>(sym("vr_lrand32")),
            range: std::mem::transmute::<*mut c_void, _>(sym("vr_lrand32_range")),
            range_count: std::mem::transmute::<*mut c_void, _>(sym("vr_lrand32_range_count")),
            drand32: std::mem::transmute::<*mut c_void, _>(sym("vr_drand32")),
            drand64: std::mem::transmute::<*mut c_void, _>(sym("vr_drand64")),
            normal: std::mem::transmute::<*mut c_void, _>(sym("vr_normal")),
            normal64: std::mem::transmute::<*mut c_void, _>(sym("vr_normal64")),
            drand64_float: std::mem::transmute::<*mut c_void, _>(sym("vr_drand64_float")),
        }
    }
}

fn fnv(h: &mut u64, w: u64) {
    for b in w.to_le_bytes() {
        *h = (*h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
}

const FNV0: u64 = 0xcbf2_9ce4_8422_2325;

#[derive(Default)]
struct Tally {
    calls: u64,
    differ: u64,
    shown: u32,
}

impl Tally {
    fn check(&mut self, what: &dyn Fn() -> String, ours: u64, theirs: u64) {
        self.calls += 1;
        if ours != theirs {
            self.differ += 1;
            if self.shown < 8 {
                self.shown += 1;
                println!("  DIFF {}: port {ours:#x} original {theirs:#x}", what());
            }
        }
    }
}

/// A range for `lrand32(a, b)`: small, large, around zero, the full width of `int` (beyond the
/// C++'s overflow only the port is defined, so those stay within `2^31 - 1`).
fn range(rng: &mut SplitMix64) -> (i32, i32) {
    match rng.below(4) {
        0 => {
            let a = rng.range(-100, 100) as i32;
            (a, a + rng.range(0, 50) as i32)
        }
        1 => (0, rng.range(0, 1 << 20) as i32),
        2 => {
            let a = rng.range(-(1 << 30), 0) as i32;
            (a, a + rng.range(0, (1 << 31) - 2) as i32)
        }
        _ => (0, i32::MAX - 1),
    }
}

/// One seed's run of the two `vnl_random`s; returns the hash of the original's outputs.
fn vnl_run(vr: &Vr, seed: u64, rng: &mut SplitMix64, t: &mut Tally, log_changes: &mut u64) -> u64 {
    // SAFETY: the shim's calls on the object it allocates (freed at the end).
    let theirs = unsafe { (vr.new)(seed) };
    let mut ours = VnlRandom::new(seed);
    let mut h = FNV0;
    for call in 0..2000 {
        let what = rng.below(12);
        let (o, c) = unsafe {
            match what {
                0 | 1 => (u64::from(ours.lrand32()), (vr.lrand32)(theirs)),
                2 => {
                    let (a, b) = range(rng);
                    (ours.lrand32_range(a, b) as u64, (vr.range)(theirs, a, b) as u64)
                }
                3 => {
                    let (a, b) = range(rng);
                    let mut count = 0;
                    let r = (vr.range_count)(theirs, a, b, &mut count);
                    let (v, n) = ours.lrand32_range_count(a, b);
                    ((v as u64) << 32 | n as u64, (r as u64) << 32 | count as u64)
                }
                4 => {
                    let a = rng.range(-1000, 1000) as f64 / 7.0;
                    let b = a + rng.below(1000) as f64 / 3.0;
                    (ours.drand32(a, b).to_bits(), (vr.drand32)(theirs, a, b).to_bits())
                }
                5 | 6 => {
                    let (a, b) = if what == 5 { (0.0, 1.0) } else { (-2.5, 7.25) };
                    (ours.drand64(a, b).to_bits(), (vr.drand64)(theirs, a, b).to_bits())
                }
                7 => {
                    let mut probe = ours.clone();
                    let r = ours.normal(math::log_glibc);
                    if probe.normal(math::log) != r {
                        *log_changes += 1;
                    }
                    (r.to_bits(), (vr.normal)(theirs).to_bits())
                }
                8 => {
                    let mut probe = ours.clone();
                    let r = ours.normal64(math::log_glibc);
                    if probe.normal64(math::log) != r {
                        *log_changes += 1;
                    }
                    (r.to_bits(), (vr.normal64)(theirs).to_bits())
                }
                9 => {
                    let v = (ours.drand64(0.0, 1.0) as f32).to_bits();
                    (u64::from(v), u64::from((vr.drand64_float)(theirs).to_bits()))
                }
                10 => {
                    if rng.below(8) == 0 {
                        ours.restart();
                        (vr.restart)(theirs);
                    } else if rng.below(8) == 0 {
                        let s = rng.next_u64();
                        ours.reseed(s);
                        (vr.reseed)(theirs, s);
                    } else if rng.below(16) == 0 {
                        let mut words: [u64; ARRAY_SIZE] = std::array::from_fn(|_| rng.next_u64() >> 32);
                        ours.reseed_array(&words);
                        (vr.reseed_array)(theirs, words.as_mut_ptr());
                    }
                    (0, 0)
                }
                _ => (u64::from(ours.lrand32()), (vr.lrand32)(theirs)),
            }
        };
        t.check(&|| format!("vnl_random seed {seed:#x} call {call} kind {what}"), o, c);
        fnv(&mut h, c);
    }
    // SAFETY: frees the shim's object.
    unsafe { (vr.free)(theirs) };
    h
}

/// The nine functions of the drand48 family after one seeding, on both.
fn drand48_run(ours: &mut Drand48, rng: &mut SplitMix64, t: &mut Tally, label: &str) -> u64 {
    let mut arr = [rng.next_u64() as u16, rng.next_u64() as u16, rng.next_u64() as u16];
    let mut theirs_arr = arr;
    let mut h = FNV0;
    for call in 0..1000 {
        let what = rng.below(6);
        // SAFETY: glibc's functions; the arrays hold three 16-bit words.
        let (o, c) = unsafe {
            match what {
                0 => (ours.drand48().to_bits(), drand48().to_bits()),
                1 => (ours.lrand48() as u64, lrand48() as u64),
                2 => (ours.mrand48() as u64, mrand48() as u64),
                3 => (ours.erand48(&mut arr).to_bits(), erand48(theirs_arr.as_mut_ptr()).to_bits()),
                4 => (ours.nrand48(&mut arr) as u64, nrand48(theirs_arr.as_mut_ptr()) as u64),
                _ => (ours.jrand48(&mut arr) as u64, jrand48(theirs_arr.as_mut_ptr()) as u64),
            }
        };
        t.check(&|| format!("drand48 {label} call {call} kind {what}"), o, c);
        let words = |a: [u16; 3]| u64::from(a[0]) | u64::from(a[1]) << 16 | u64::from(a[2]) << 32;
        t.check(&|| format!("drand48 {label} call {call} array"), words(arr), words(theirs_arr));
        fnv(&mut h, c);
    }
    h
}

/// The seeds of the recorded fixed sequences.
const DRAND48_SEEDS: [i64; 5] = [53, 0, -1, 1234, 0x1234_5678_9abc];
const VNL_SEEDS: [u64; 5] = [1234, 53, 0, 1, u64::MAX];

/// The fixed drand48 sequence the unit tests repeat: `srand48(seed)`, 300 `drand48`, 300
/// `lrand48`, 300 `mrand48`, 100 each of `erand48`, `nrand48`, `jrand48` on the array
/// `[1, 2, 3]`, `seed48([0x1234, 0x5678, 0x9abc])` (its previous state hashed), 100 `drand48`,
/// `lcong48([5, 6, 7, 0x1111, 0x2222, 0x3333, 0x44])`, 100 `drand48` and 100 `erand48`.
fn drand48_sequence(seed: i64) -> u64 {
    let mut s = Drand48::default();
    let mut h = FNV0;
    s.srand48(seed);
    for _ in 0..300 {
        fnv(&mut h, s.drand48().to_bits());
    }
    for _ in 0..300 {
        fnv(&mut h, s.lrand48() as u64);
    }
    for _ in 0..300 {
        fnv(&mut h, s.mrand48() as u64);
    }
    let mut a = [1u16, 2, 3];
    for _ in 0..100 {
        fnv(&mut h, s.erand48(&mut a).to_bits());
    }
    for _ in 0..100 {
        fnv(&mut h, s.nrand48(&mut a) as u64);
    }
    for _ in 0..100 {
        fnv(&mut h, s.jrand48(&mut a) as u64);
    }
    let old = s.seed48([0x1234, 0x5678, 0x9abc]);
    fnv(&mut h, u64::from(old[0]) | u64::from(old[1]) << 16 | u64::from(old[2]) << 32);
    for _ in 0..100 {
        fnv(&mut h, s.drand48().to_bits());
    }
    s.lcong48([5, 6, 7, 0x1111, 0x2222, 0x3333, 0x44]);
    for _ in 0..100 {
        fnv(&mut h, s.drand48().to_bits());
    }
    for _ in 0..100 {
        fnv(&mut h, s.erand48(&mut a).to_bits());
    }
    h
}

/// The same with the C library.
unsafe fn glibc_sequence(seed: i64) -> u64 {
    let mut h = FNV0;
    unsafe {
        srand48(seed);
        for _ in 0..300 {
            fnv(&mut h, drand48().to_bits());
        }
        for _ in 0..300 {
            fnv(&mut h, lrand48() as u64);
        }
        for _ in 0..300 {
            fnv(&mut h, mrand48() as u64);
        }
        let mut a = [1u16, 2, 3];
        for _ in 0..100 {
            fnv(&mut h, erand48(a.as_mut_ptr()).to_bits());
        }
        for _ in 0..100 {
            fnv(&mut h, nrand48(a.as_mut_ptr()) as u64);
        }
        for _ in 0..100 {
            fnv(&mut h, jrand48(a.as_mut_ptr()) as u64);
        }
        let mut w = [0x1234u16, 0x5678, 0x9abc];
        let old = std::slice::from_raw_parts(seed48(w.as_mut_ptr()), 3);
        fnv(&mut h, u64::from(old[0]) | u64::from(old[1]) << 16 | u64::from(old[2]) << 32);
        for _ in 0..100 {
            fnv(&mut h, drand48().to_bits());
        }
        let mut p = [5u16, 6, 7, 0x1111, 0x2222, 0x3333, 0x44];
        lcong48(p.as_mut_ptr());
        for _ in 0..100 {
            fnv(&mut h, drand48().to_bits());
        }
        for _ in 0..100 {
            fnv(&mut h, erand48(a.as_mut_ptr()).to_bits());
        }
    }
    h
}

/// The fixed vnl_random sequence the unit tests repeat: `vnl_random(seed)`, 1,000 `lrand32`,
/// 500 `drand64(0, 1)`, 200 `normal64` and 200 `normal` (glibc's logarithm), 300
/// `(float) drand64(0, 1)`, 200 `lrand32(-5, 1000)`, 200 `drand32(-1, 1)`.
fn vnl_sequence(seed: u64) -> u64 {
    let mut r = VnlRandom::new(seed);
    let mut h = FNV0;
    for _ in 0..1000 {
        fnv(&mut h, u64::from(r.lrand32()));
    }
    for _ in 0..500 {
        fnv(&mut h, r.drand64(0.0, 1.0).to_bits());
    }
    for _ in 0..200 {
        fnv(&mut h, r.normal64(math::log_glibc).to_bits());
    }
    for _ in 0..200 {
        fnv(&mut h, r.normal(math::log_glibc).to_bits());
    }
    for _ in 0..300 {
        fnv(&mut h, u64::from((r.drand64(0.0, 1.0) as f32).to_bits()));
    }
    for _ in 0..200 {
        fnv(&mut h, r.lrand32_range(-5, 1000) as u64);
    }
    for _ in 0..200 {
        fnv(&mut h, r.drand32(-1.0, 1.0).to_bits());
    }
    h
}

/// The same with ITK's vnl_random.
fn vnl_original_sequence(vr: &Vr, seed: u64) -> u64 {
    let mut h = FNV0;
    // SAFETY: the shim's calls on the object it allocates (freed at the end).
    unsafe {
        let r = (vr.new)(seed);
        for _ in 0..1000 {
            fnv(&mut h, (vr.lrand32)(r));
        }
        for _ in 0..500 {
            fnv(&mut h, (vr.drand64)(r, 0.0, 1.0).to_bits());
        }
        for _ in 0..200 {
            fnv(&mut h, (vr.normal64)(r).to_bits());
        }
        for _ in 0..200 {
            fnv(&mut h, (vr.normal)(r).to_bits());
        }
        for _ in 0..300 {
            fnv(&mut h, u64::from((vr.drand64_float)(r).to_bits()));
        }
        for _ in 0..200 {
            fnv(&mut h, (vr.range)(r, -5, 1000) as u64);
        }
        for _ in 0..200 {
            fnv(&mut h, (vr.drand32)(r, -1.0, 1.0).to_bits());
        }
        (vr.free)(r);
    }
    h
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let lib = args.get(1).expect("usage: fs-random <libvnlrandom.so> [n_seeds] [--record <file>]");
    let mut n_seeds = 2000usize;
    let mut record: Option<String> = None;
    let mut i = 2;
    while i < args.len() {
        if args[i] == "--record" {
            record = Some(args[i + 1].clone());
            i += 2;
        } else {
            n_seeds = args[i].parse().expect("n_seeds");
            i += 1;
        }
    }
    let mut rec = String::new();
    writeln!(rec, "// Recorded by validation/probes/fs-random (fs-random --record).").unwrap();

    // drand48 first: the state before any seeding (this process has not called the family yet).
    let mut t48 = Tally::default();
    let mut ours = Drand48::default();
    for k in 0..16 {
        // SAFETY: glibc's drand48.
        let c = unsafe { drand48() };
        t48.check(&|| format!("drand48 unseeded call {k}"), ours.drand48().to_bits(), c.to_bits());
    }
    let mut rng = SplitMix64::new(0x0048);
    for k in 0..n_seeds {
        let seed = match k {
            0 => 53,
            1 => 0,
            2 => -1,
            3 => 1234,
            _ => rng.next_u64() as i64,
        };
        // SAFETY: glibc's srand48.
        unsafe { srand48(seed) };
        ours.srand48(seed);
        let h = drand48_run(&mut ours, &mut SplitMix64::new(seed as u64), &mut t48, &format!("srand48({seed})"));
        let _ = h;
        // seed48 and lcong48 between seedings.
        let mut words = [rng.next_u64() as u16, rng.next_u64() as u16, rng.next_u64() as u16];
        // SAFETY: glibc's seed48 returns a pointer to its three-word buffer.
        let old = unsafe { std::slice::from_raw_parts(seed48(words.as_mut_ptr()), 3).to_vec() };
        let ours_old = ours.seed48(words);
        let pack = |a: &[u16]| u64::from(a[0]) | u64::from(a[1]) << 16 | u64::from(a[2]) << 32;
        t48.check(&|| format!("seed48 previous after srand48({seed})"), pack(&ours_old), pack(&old));
        drand48_run(&mut ours, &mut SplitMix64::new(seed as u64 ^ 1), &mut t48, "seed48");
        let mut param: [u16; 7] = std::array::from_fn(|_| rng.next_u64() as u16);
        if k % 2 == 0 {
            param[3..6].copy_from_slice(&[0xe66d, 0xdeec, 0x5]);
        }
        // SAFETY: glibc's lcong48 reads seven words.
        unsafe { lcong48(param.as_mut_ptr()) };
        ours.lcong48(param);
        drand48_run(&mut ours, &mut SplitMix64::new(seed as u64 ^ 2), &mut t48, "lcong48");
    }
    println!("drand48 family: {} calls, {} differ", t48.calls, t48.differ);
    writeln!(rec, "// drand48: the C library's outputs of drand48_sequence(seed) (tests.rs).").unwrap();
    writeln!(rec, "pub(super) const DRAND48_HASHES: [(i64, u64); 5] = [").unwrap();
    for seed in DRAND48_SEEDS {
        // SAFETY: glibc's functions; the arrays hold three and seven 16-bit words.
        let h = unsafe { glibc_sequence(seed) };
        let mine = drand48_sequence(seed);
        t48.check(&|| format!("drand48 fixed sequence of seed {seed}"), mine, h);
        writeln!(rec, "    ({seed}, {h:#018x}),").unwrap();
    }
    writeln!(rec, "];").unwrap();

    // vnl_random.
    let vr = load(lib);
    let mut tv = Tally::default();
    let mut log_changes = 0u64;
    let mut rng = SplitMix64::new(0x0072);
    for k in 0..n_seeds {
        let seed = match k {
            0 => 1234,
            1 => 53,
            2 => 0,
            3 => 1,
            4 => u64::MAX,
            _ => rng.next_u64(),
        };
        let h = vnl_run(&vr, seed, &mut SplitMix64::new(seed ^ 0x72), &mut tv, &mut log_changes);
        let _ = h;
    }
    writeln!(rec, "// vnl_random: the original's outputs of vnl_sequence(seed) (vnl_random.rs's tests).").unwrap();
    writeln!(rec, "pub(super) const VNL_HASHES: [(u64, u64); 5] = [").unwrap();
    for seed in VNL_SEEDS {
        let h = vnl_original_sequence(&vr, seed);
        tv.check(&|| format!("vnl_random fixed sequence of seed {seed:#x}"), vnl_sequence(seed), h);
        writeln!(rec, "    ({seed:#x}, {h:#018x}),").unwrap();
    }
    writeln!(rec, "];").unwrap();
    println!(
        "vnl_random: {} calls, {} differ; the correctly rounded log changes {log_changes} normal numbers",
        tv.calls, tv.differ
    );
    if let Some(path) = record {
        std::fs::write(&path, rec).expect("write the record");
        println!("hashes written to {path}");
    }
    if t48.differ + tv.differ == 0 {
        println!("VERIFY-PASS");
    } else {
        println!("VERIFY-FAIL");
        std::process::exit(1);
    }
}
