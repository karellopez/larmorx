// SPDX-License-Identifier: Apache-2.0
//! `larmorx_core::math::cbrtf_glibc` (the clean-room cube root with glibc 2.35's bits,
//! `specs/cbrtf.md`) against the system's `cbrtf`, run as a black box (the C library's
//! `libm.so.6`, opened with `dlopen`), on every one of the 2^32 float inputs.
//!
//! - `verify <blocks.json> [--threads N] [--out result.json]`: every input compared bit for
//!   bit (NaN results included); the spec's block hashes (§7.1: 4096 blocks of 2^20 inputs,
//!   FNV-1a over the 32-bit results, a NaN counted as `0x7fc00000`) of both functions against
//!   the committed hashes of glibc 2.35; the hash of the whole sequence (§7.2); the hash of
//!   every 4099th input (the unit test's).
//! - `list <first input>`: one block, input by input, both functions.
//! - `bench`: nanoseconds per call of the three cube roots (this one, glibc's, CORE-MATH's).
//!
//! The system's function is only called; its code is never read.

use std::collections::BTreeMap;
use std::ffi::{CStr, c_char, c_int, c_void};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

use larmorx_core::math::{cbrtf, cbrtf_glibc};

const BLOCK: u64 = 1 << 20;
const BLOCKS: usize = 4096;
const FNV_START: u64 = 14695981039346656037;
const FNV_PRIME: u64 = 1099511628211;

unsafe extern "C" {
    fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlerror() -> *const c_char;
    fn gnu_get_libc_version() -> *const c_char;
}

const RTLD_NOW: c_int = 2;

type Cbrtf = extern "C" fn(f32) -> f32;

/// The system's `cbrtf`, from `libm.so.6`.
fn system_cbrtf() -> Cbrtf {
    // SAFETY: plain calls of the C library's dynamic loader with NUL-terminated names; the
    // symbol is the C function `float cbrtf(float)`.
    unsafe {
        let h = dlopen(c"libm.so.6".as_ptr(), RTLD_NOW);
        if h.is_null() {
            panic!("dlopen(libm.so.6): {:?}", CStr::from_ptr(dlerror()));
        }
        let f = dlsym(h, c"cbrtf".as_ptr());
        if f.is_null() {
            panic!("dlsym(cbrtf): {:?}", CStr::from_ptr(dlerror()));
        }
        std::mem::transmute::<*mut c_void, Cbrtf>(f)
    }
}

fn libc_version() -> String {
    // SAFETY: glibc's function returns a static NUL-terminated string.
    unsafe { CStr::from_ptr(gnu_get_libc_version()) }
        .to_string_lossy()
        .into_owned()
}

/// A result as the spec's hash counts it: every NaN as `0x7fc00000`.
#[inline]
fn hashed(b: u32) -> u32 {
    if b & 0x7fff_ffff > 0x7f80_0000 {
        0x7fc0_0000
    } else {
        b
    }
}

#[inline]
fn fnv(h: u64, b: u32) -> u64 {
    (h ^ u64::from(hashed(b))).wrapping_mul(FNV_PRIME)
}

fn is_nan(b: u32) -> bool {
    b & 0x7fff_ffff > 0x7f80_0000
}

/// The committed block hashes: `{"0x0000": "8145ce529bb604f1", ...}` read without a parser.
fn read_blocks(path: &str) -> Vec<u64> {
    let text = std::fs::read_to_string(path).expect("the block hashes");
    let body = &text[text.find("\"blocks\"").expect("a blocks table")..];
    let mut out = vec![None; BLOCKS];
    let mut rest = body;
    while let Some(i) = rest.find("\"0x") {
        let key = &rest[i + 1..i + 7];
        let after = &rest[i + 8..]; // past the key's closing quote
        let v0 = after.find('"').unwrap() + 1;
        let v1 = v0 + after[v0..].find('"').unwrap();
        let index = usize::from_str_radix(&key[2..], 16).unwrap();
        out[index] = Some(u64::from_str_radix(&after[v0..v1], 16).unwrap());
        rest = &after[v1 + 1..];
    }
    out.into_iter().map(|v| v.expect("4096 blocks")).collect()
}

fn hash_of_hashes_fields(text: &str) -> Option<String> {
    let i = text.find("\"hash_of_block_hashes\"")?;
    let rest = &text[i + 22..];
    let v0 = rest.find('"')? + 1;
    let v1 = v0 + rest[v0..].find('"')?;
    Some(rest[v0..v1].to_string())
}

struct Mismatch {
    input: u32,
    ours: u32,
    glibc: u32,
}

#[derive(Default)]
struct Totals {
    inputs: u64,
    identical: u64,
    nan_inputs: u64,
    nan_identical: u64,
    glibc_not_cr: u64,
}

fn verify(blocks_path: &str, threads: usize, out: Option<&str>) {
    let expected = read_blocks(blocks_path);
    let text = std::fs::read_to_string(blocks_path).unwrap();
    let f = system_cbrtf();
    let next = AtomicUsize::new(0);
    let hashed_upto = AtomicUsize::new(0);
    let pending: Mutex<BTreeMap<usize, Vec<u32>>> = Mutex::new(BTreeMap::new());
    let ours_hashes: Vec<AtomicU64> = (0..BLOCKS).map(|_| AtomicU64::new(0)).collect();
    let glibc_hashes: Vec<AtomicU64> = (0..BLOCKS).map(|_| AtomicU64::new(0)).collect();
    let totals = Mutex::new(Totals::default());
    let mismatches: Mutex<Vec<Mismatch>> = Mutex::new(Vec::new());
    let start = Instant::now();
    let mut whole = FNV_START;
    let mut whole_raw = FNV_START;
    let mut whole_bytes = FNV_START;
    let mut whole_bytes_raw = FNV_START;
    let mut stride = FNV_START;
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                let mut ours = vec![0u32; BLOCK as usize];
                loop {
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    if k >= BLOCKS {
                        break;
                    }
                    // Keep at most 64 blocks waiting for the sequential hash.
                    while k > hashed_upto.load(Ordering::Acquire) + 64 {
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                    let mut glibc = vec![0u32; BLOCK as usize];
                    let mut t = Totals::default();
                    let (mut ho, mut hg) = (FNV_START, FNV_START);
                    let mut bad = Vec::new();
                    for j in 0..BLOCK {
                        let i = (k as u64 * BLOCK + j) as u32;
                        let x = f32::from_bits(i);
                        let a = cbrtf_glibc(x).to_bits();
                        let b = f(x).to_bits();
                        ours[j as usize] = a;
                        glibc[j as usize] = b;
                        ho = fnv(ho, a);
                        hg = fnv(hg, b);
                        t.inputs += 1;
                        if a == b {
                            t.identical += 1;
                        } else if bad.len() < 32 {
                            bad.push(Mismatch {
                                input: i,
                                ours: a,
                                glibc: b,
                            });
                        }
                        if is_nan(i) {
                            t.nan_inputs += 1;
                            if a == b {
                                t.nan_identical += 1;
                            }
                        } else if b != cbrtf(x).to_bits() {
                            t.glibc_not_cr += 1;
                        }
                    }
                    ours_hashes[k].store(ho, Ordering::Relaxed);
                    glibc_hashes[k].store(hg, Ordering::Relaxed);
                    {
                        let mut all = totals.lock().unwrap();
                        all.inputs += t.inputs;
                        all.identical += t.identical;
                        all.nan_inputs += t.nan_inputs;
                        all.nan_identical += t.nan_identical;
                        all.glibc_not_cr += t.glibc_not_cr;
                    }
                    mismatches.lock().unwrap().extend(bad);
                    pending.lock().unwrap().insert(k, glibc);
                }
            });
        }
        // The whole sequence's hash (§7.2) and the stride's, in input order.
        let mut k = 0;
        while k < BLOCKS {
            let block = pending.lock().unwrap().remove(&k);
            match block {
                Some(v) => {
                    for (j, &b) in v.iter().enumerate() {
                        whole = fnv(whole, b);
                        whole_raw = (whole_raw ^ u64::from(b)).wrapping_mul(FNV_PRIME);
                        for (h, w) in [(&mut whole_bytes, hashed(b)), (&mut whole_bytes_raw, b)] {
                            for byte in w.to_le_bytes() {
                                *h = (*h ^ u64::from(byte)).wrapping_mul(FNV_PRIME);
                            }
                        }
                        let i = k as u64 * BLOCK + j as u64;
                        if i.is_multiple_of(4099) {
                            stride = fnv(stride, b);
                        }
                    }
                    k += 1;
                    hashed_upto.store(k, Ordering::Release);
                }
                None => std::thread::sleep(std::time::Duration::from_millis(1)),
            }
        }
    });
    let seconds = start.elapsed().as_secs_f64();
    let t = totals.into_inner().unwrap();
    let mut m = mismatches.into_inner().unwrap();
    m.sort_by_key(|x| x.input);
    let ours: Vec<u64> = ours_hashes
        .iter()
        .map(|h| h.load(Ordering::Relaxed))
        .collect();
    let glibc: Vec<u64> = glibc_hashes
        .iter()
        .map(|h| h.load(Ordering::Relaxed))
        .collect();
    let ours_ok = (0..BLOCKS).filter(|&k| ours[k] == expected[k]).count();
    let glibc_ok = (0..BLOCKS).filter(|&k| glibc[k] == expected[k]).count();
    let differing: Vec<String> = (0..BLOCKS)
        .filter(|&k| ours[k] != expected[k])
        .map(|k| format!("{k:#06x}"))
        .collect();
    // Two guesses at how the committed hash of the block hashes was formed (informative only:
    // every block hash is compared above).
    let mut g1 = FNV_START;
    let mut g2 = FNV_START;
    for &h in &ours {
        g1 = (g1 ^ h).wrapping_mul(FNV_PRIME);
        for b in format!("{h:016x}").bytes() {
            g2 = (g2 ^ u64::from(b)).wrapping_mul(FNV_PRIME);
        }
    }
    let committed = hash_of_hashes_fields(&text).unwrap_or_default();
    let guess = if format!("{g1:016x}") == committed {
        "FNV-1a over the 4096 block hashes as 64-bit words: equal"
    } else if format!("{g2:016x}") == committed {
        "FNV-1a over the block hashes' hexadecimal text: equal"
    } else {
        "not reproduced by the two guesses (informative only)"
    };
    let result = format!(
        "{{\n  \"libc\": \"glibc {libc}\",\n  \"machine\": \"{arch}\",\n  \"threads\": {threads},\n  \
         \"seconds\": {seconds:.1},\n  \"inputs\": {inputs},\n  \"identical_bit_for_bit\": {ident},\n  \
         \"differing\": {diff},\n  \"nan_inputs\": {nan},\n  \"nan_identical_bit_for_bit\": {nanid},\n  \
         \"glibc_not_correctly_rounded\": {ncr},\n  \"blocks\": {BLOCKS},\n  \
         \"blocks_ours_equal_committed\": {ours_ok},\n  \"blocks_system_equal_committed\": {glibc_ok},\n  \
         \"blocks_differing\": [{bd}],\n  \"whole_sequence_hash\": \"{whole:016x}\",\n  \
         \"whole_sequence_hash_spec\": \"c679cb92d4402e33\",\n  \
         \"whole_sequence_hash_nan_bits_kept\": \"{whole_raw:016x}\",\n  \
         \"whole_sequence_hash_bytewise\": \"{whole_bytes:016x}\",\n  \
         \"whole_sequence_hash_bytewise_nan_bits_kept\": \"{whole_bytes_raw:016x}\",\n  \"stride_4099_hash\": \"{stride:016x}\",\n  \
         \"hash_of_block_hashes_committed\": \"{committed}\",\n  \"hash_of_block_hashes_guess\": \"{guess}\",\n  \
         \"first_mismatches\": [{mm}]\n}}\n",
        libc = libc_version(),
        arch = std::env::consts::ARCH,
        inputs = t.inputs,
        ident = t.identical,
        diff = t.inputs - t.identical,
        nan = t.nan_inputs,
        nanid = t.nan_identical,
        ncr = t.glibc_not_cr,
        bd = differing.join(", "),
        mm = m
            .iter()
            .take(32)
            .map(|x| format!(
                "{{\"input\": \"{:#010x}\", \"ours\": \"{:#010x}\", \"glibc\": \"{:#010x}\"}}",
                x.input, x.ours, x.glibc
            ))
            .collect::<Vec<_>>()
            .join(", "),
    );
    print!("{result}");
    if let Some(path) = out {
        std::fs::write(path, &result).expect("the result file");
    }
    // The proof: every input bit for bit, and every committed block hash. The whole
    // sequence's hash is reported beside the spec's number (§7.2), which the system's own
    // results do not give under §7.1's definition (README).
    let ok = t.identical == 1 << 32 && ours_ok == BLOCKS && glibc_ok == BLOCKS;
    println!("{}", if ok { "VERIFY-PASS" } else { "VERIFY-FAIL" });
    if !ok {
        std::process::exit(1);
    }
}

fn list(first: &str) {
    let first = u32::from_str_radix(first.trim_start_matches("0x"), 16).expect("hex input");
    let f = system_cbrtf();
    let start = first & !((BLOCK as u32) - 1);
    let mut n = 0;
    for j in 0..BLOCK as u32 {
        let x = f32::from_bits(start + j);
        let (a, b) = (cbrtf_glibc(x).to_bits(), f(x).to_bits());
        if a != b {
            println!("{:#010x} ours {a:#010x} glibc {b:#010x}", start + j);
            n += 1;
        }
    }
    println!("{n} of {BLOCK} differ in the block from {start:#010x}");
}

fn bench() {
    let f = system_cbrtf();
    let n = 1u32 << 26;
    // Normal inputs over a range of exponents (0.5 .. 2^24 and their negatives' bit patterns).
    let inputs: Vec<f32> = (0..n)
        .map(|i| f32::from_bits(0x3f00_0000u32.wrapping_add(i.wrapping_mul(13))))
        .collect();
    let time = |name: &str, g: &dyn Fn(f32) -> f32| {
        let mut best = f64::INFINITY;
        for _ in 0..5 {
            let t = Instant::now();
            let mut acc = 0u32;
            for &x in &inputs {
                acc ^= std::hint::black_box(g(std::hint::black_box(x))).to_bits();
            }
            std::hint::black_box(acc);
            best = best.min(t.elapsed().as_secs_f64() * 1e9 / f64::from(n));
        }
        println!("{name}: {best:.2} ns per call (best of 5, {n} inputs, one thread)");
    };
    time("cbrtf_glibc (larmorx, clean-room)", &|x| cbrtf_glibc(x));
    time("cbrtf (system libm, through a pointer)", &|x| f(x));
    time("cbrtf (larmorx, CORE-MATH, correctly rounded)", &|x| {
        cbrtf(x)
    });
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("verify") => {
            let path = args.get(2).expect("the block hashes' JSON");
            let mut threads = 8;
            let mut out = None;
            let mut i = 3;
            while i < args.len() {
                match args[i].as_str() {
                    "--threads" => {
                        threads = args[i + 1].parse().unwrap();
                        i += 2;
                    }
                    "--out" => {
                        out = Some(args[i + 1].clone());
                        i += 2;
                    }
                    other => panic!("unknown argument {other}"),
                }
            }
            verify(path, threads, out.as_deref());
        }
        Some("list") => list(args.get(2).expect("the first input, hexadecimal")),
        Some("bench") => bench(),
        _ => {
            eprintln!(
                "usage: cbrtf-glibc-verify verify <blocks.json> [--threads N] [--out FILE] | list <hex> | bench"
            );
            std::process::exit(2);
        }
    }
}
