// SPDX-License-Identifier: Apache-2.0
//! The clean room of the C library's `qsort` and `qsort_r` (`larmorx_core::glibc_qsort`,
//! `specs/glibc-qsort.md`) behind the input and output lines of the black-box probe
//! `validation/investigations/glibc-qsort-probe/glibc_qsort_probe.c`: `qsort [--bytes] [--order
//! glibc|merge|quick]` reads the probe's cases `s n mode seed keyrange api fail` on standard input
//! and prints the probe's line for each (`calls hash sorted requests errno`, or `calls hash
//! outside`), with the probe's elements, comparator kinds and hashes (the spec's §10).
//!
//! `--order glibc` (the default) takes §3's choice on this machine (4096-byte pages, the physical
//! pages from `/proc/meminfo`, the allocation refused when `fail` is 1); `merge` and `quick` force
//! one order (the requests column is then 0). At the end, standard error counts the cases that
//! stopped outside the array by the error's kind (a position, a reversed range). The typed functions sort `[u8; s]` elements for the
//! sizes the validation uses (`--bytes`: always the byte functions); `api` `qsort_r` sorts through
//! `qsort_r` (the merge order) or a closure holding the context, which every call checks.
//!
//! `qsort-bench <s> <n> <mode> <seed> <keyrange> <repeats>` times the merge order on one case with
//! a plain comparator (no counting or hashing) and prints the best time.

use larmorx_core::glibc_qsort::{
    Choice, Machine, Order, QuickOrderError, choose, merge_order, merge_order_bytes,
    qsort_as_glibc, qsort_as_glibc_bytes, qsort_r, quick_order, quick_order_bytes,
};
use std::io::{BufRead, Write};
use std::time::Instant;

struct SplitMix(u64);

impl SplitMix {
    fn mix(mut z: u64) -> u64 {
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        SplitMix::mix(self.0)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    ThreeWay,
    TiePlus,
    TieMinus,
    Hash,
    F3,
    F2,
    D3,
    D2,
}

fn kind_of(s: &str) -> Option<Kind> {
    Some(match s {
        "3way" => Kind::ThreeWay,
        "tie+" => Kind::TiePlus,
        "tie-" => Kind::TieMinus,
        "hash" => Kind::Hash,
        "f3" => Kind::F3,
        "f2" => Kind::F2,
        "d3" => Kind::D3,
        "d2" => Kind::D2,
        _ => return None,
    })
}

const FVALS: [u32; 12] = [
    0x7fc0_0000,
    0x8000_0000,
    0,
    0x3f80_0000,
    0xbf80_0000,
    0x7f80_0000,
    0xff80_0000,
    0x7fc0_0001,
    0xffc0_0000,
    0x4000_0000,
    1,
    0x3f80_0000,
];
const DVALS: [u64; 12] = [
    0x7ff8_0000_0000_0000,
    0x8000_0000_0000_0000,
    0,
    0x3ff0_0000_0000_0000,
    0xbff0_0000_0000_0000,
    0x7ff0_0000_0000_0000,
    0xfff0_0000_0000_0000,
    0x7ff8_0000_0000_0001,
    0xfff8_0000_0000_0000,
    0x4000_0000_0000_0000,
    1,
    0x3ff0_0000_0000_0000,
];

fn elements(s: usize, n: usize, kind: Kind, seed: u64, keyrange: u64) -> Vec<u8> {
    let mut g = SplitMix(seed);
    let mut data = vec![0u8; n * s];
    for i in 0..n {
        let p = &mut data[i * s..(i + 1) * s];
        let k = g.next() % keyrange;
        match kind {
            Kind::F3 | Kind::F2 => p.copy_from_slice(&FVALS[(k % 12) as usize].to_le_bytes()),
            Kind::D3 | Kind::D2 => p.copy_from_slice(&DVALS[(k % 12) as usize].to_le_bytes()),
            _ if s == 1 => p[0] = k as u8,
            _ if s == 2 => p.copy_from_slice(&(k as u16).to_le_bytes()),
            _ => {
                p[..4].copy_from_slice(&(k as u32).to_le_bytes());
                if s >= 8 {
                    p[4..8].copy_from_slice(&(i as u32).to_le_bytes());
                    let mut off = 8;
                    while off < s {
                        let r = g.next().to_le_bytes();
                        let m = (s - off).min(8);
                        p[off..off + m].copy_from_slice(&r[..m]);
                        off += 8;
                    }
                }
            }
        }
    }
    data
}

fn key(e: &[u8]) -> u32 {
    match e.len() {
        1 => u32::from(e[0]),
        2 => u32::from(u16::from_le_bytes([e[0], e[1]])),
        _ => u32::from_le_bytes([e[0], e[1], e[2], e[3]]),
    }
}

fn three(lt: bool, gt: bool) -> i32 {
    if lt {
        -1
    } else if gt {
        1
    } else {
        0
    }
}

fn compare(kind: Kind, hseed: u64, a: &[u8], b: &[u8]) -> i32 {
    match kind {
        Kind::ThreeWay => three(key(a) < key(b), key(a) > key(b)),
        Kind::TiePlus => {
            if key(a) < key(b) {
                -1
            } else {
                1
            }
        }
        Kind::TieMinus => {
            if key(a) > key(b) {
                1
            } else {
                -1
            }
        }
        Kind::Hash => {
            let ta = u64::from(u32::from_le_bytes([a[4], a[5], a[6], a[7]]));
            let tb = u64::from(u32::from_le_bytes([b[4], b[5], b[6], b[7]]));
            let z = hseed
                ^ ta.wrapping_mul(0x9E37_79B9_7F4A_7C15)
                ^ tb.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
            (SplitMix::mix(z) % 3) as i32 - 1
        }
        Kind::F3 | Kind::F2 => {
            let x = f32::from_le_bytes([a[0], a[1], a[2], a[3]]);
            let y = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
            three(x < y, if kind == Kind::F3 { x > y } else { x != y })
        }
        Kind::D3 | Kind::D2 => {
            let x = f64::from_le_bytes(a[..8].try_into().unwrap());
            let y = f64::from_le_bytes(b[..8].try_into().unwrap());
            three(x < y, if kind == Kind::D3 { x > y } else { x != y })
        }
    }
}

const FNV0: u64 = 14_695_981_039_346_656_037;
const FNVP: u64 = 1_099_511_628_211;

fn fnv(mut h: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        h = (h ^ u64::from(b)).wrapping_mul(FNVP);
    }
    h
}

struct Recorder {
    kind: Kind,
    hseed: u64,
    calls: u64,
    hash: u64,
    context_lost: bool,
}

impl Recorder {
    fn call(&mut self, a: &[u8], b: &[u8]) -> i32 {
        let m = a.len().min(8);
        self.calls += 1;
        self.hash = fnv(fnv(self.hash, &a[..m]), &b[..m]);
        compare(self.kind, self.hseed, a, b)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Force {
    Glibc,
    Merge,
    Quick,
}

/// The context `qsort_r` passes: every call checks it is this one.
const TOKEN: u64 = 0x5EED_C0DE;

/// One case through the byte functions: the sort's result and the allocator requests.
fn sort_bytes(
    data: &mut [u8],
    s: usize,
    rec: &mut Recorder,
    force: Force,
    machine: &Machine,
    with_context: bool,
) -> Result<u32, QuickOrderError> {
    let mut ctx = TOKEN;
    let mut cmp = |a: &[u8], b: &[u8]| {
        if with_context && ctx != TOKEN {
            rec.context_lost = true;
        }
        ctx = TOKEN;
        rec.call(a, b)
    };
    match force {
        Force::Glibc => qsort_as_glibc_bytes(data, s, &mut cmp, machine).map(Choice::requests),
        Force::Merge => {
            merge_order_bytes(data, s, &mut cmp);
            Ok(0)
        }
        Force::Quick => quick_order_bytes(data, s, &mut cmp).map(|()| 0),
    }
}

/// One case through the typed functions on `[u8; S]` elements.
fn sort_typed<const S: usize>(
    data: &mut [u8],
    rec: &mut Recorder,
    force: Force,
    machine: &Machine,
    with_context: bool,
) -> Result<u32, QuickOrderError> {
    let mut v: Vec<[u8; S]> = data
        .chunks(S)
        .map(|c| <[u8; S]>::try_from(c).unwrap())
        .collect();
    let choice = choose(v.len(), S, machine);
    let order = match force {
        Force::Glibc => choice.order(),
        Force::Merge => Order::Merge,
        Force::Quick => Order::Quick,
    };
    let result = if with_context && order == Order::Merge {
        // the merge order through `qsort_r`, the context checked in every call
        let mut ctx = TOKEN;
        qsort_r(&mut v, &mut ctx, |a: &[u8; S], b: &[u8; S], c: &mut u64| {
            if *c != TOKEN {
                rec.context_lost = true;
            }
            rec.call(a, b)
        });
        Ok(if force == Force::Glibc {
            choice.requests()
        } else {
            0
        })
    } else {
        let mut cmp = |a: &[u8; S], b: &[u8; S]| rec.call(a, b);
        match force {
            Force::Glibc => qsort_as_glibc(&mut v, &mut cmp, machine).map(Choice::requests),
            Force::Merge => {
                merge_order(&mut v, &mut cmp);
                Ok(0)
            }
            Force::Quick => quick_order(&mut v, &mut cmp).map(|()| 0),
        }
    };
    data.copy_from_slice(&v.concat());
    result
}

macro_rules! typed_sizes {
    ($s:expr, $data:expr, $rec:expr, $force:expr, $machine:expr, $ctx:expr; $($n:literal)*) => {
        match $s {
            $($n => Some(sort_typed::<$n>($data, $rec, $force, $machine, $ctx)),)*
            _ => None,
        }
    };
}

/// The physical pages as `sysconf(_SC_PHYS_PAGES)` reports them (`MemTotal` in 4096-byte pages).
fn phys_pages() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let line = text.lines().find(|l| l.starts_with("MemTotal:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kib / 4)
}

pub fn run(args: &[String], out: &mut impl Write) -> Result<(), String> {
    let mut bytes = false;
    let mut force = Force::Glibc;
    let mut a = args;
    loop {
        match a.first().map(String::as_str) {
            Some("--bytes") => bytes = true,
            Some("--order") if a.len() > 1 => {
                force = match a[1].as_str() {
                    "glibc" => Force::Glibc,
                    "merge" => Force::Merge,
                    "quick" => Force::Quick,
                    o => return Err(format!("unknown order {o}")),
                };
                a = &a[1..];
            }
            Some(o) => return Err(format!("unknown option {o}")),
            None => break,
        }
        a = &a[1..];
    }
    let phys = phys_pages();
    // the two ways the fallback stops (printed on standard error at the end)
    let (mut at_position, mut reversed) = (0u64, 0u64);
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let line = line.map_err(|e| e.to_string())?;
        let w: Vec<&str> = line.split_whitespace().collect();
        if w.len() != 7 {
            continue;
        }
        let num = |i: usize| w[i].parse::<u64>().map_err(|e| format!("{line}: {e}"));
        let (s, n, seed, keyrange) = (num(0)? as usize, num(1)? as usize, num(3)?, num(4)?);
        let fail = w[6] == "1";
        let kind = kind_of(w[2]);
        let bad = match kind {
            None => true,
            Some(k) => {
                s == 0
                    || keyrange == 0
                    || (k == Kind::Hash && s < 8)
                    || (matches!(k, Kind::F3 | Kind::F2) && s != 4)
                    || (matches!(k, Kind::D3 | Kind::D2) && s != 8)
            }
        };
        if bad {
            let _ = writeln!(out, "bad case");
            continue;
        }
        let kind = kind.unwrap();
        let with_context = w[5] == "qsort_r";
        let machine = Machine {
            page_size: 4096,
            phys_pages: phys,
            allocation_succeeds: !fail,
        };
        let mut data = elements(s, n, kind, seed, keyrange);
        let mut rec = Recorder {
            kind,
            hseed: seed ^ 0x5_DEEC_E66D,
            calls: 0,
            hash: FNV0,
            context_lost: false,
        };
        let typed = if bytes {
            None
        } else {
            typed_sizes!(s, &mut data, &mut rec, force, &machine, with_context;
                1 2 4 8 12 16 24 32 33 40 72 104 160 1024 1040)
        };
        let result = match typed {
            Some(r) => r,
            None => sort_bytes(&mut data, s, &mut rec, force, &machine, with_context),
        };
        let lost = if rec.context_lost {
            " context-lost"
        } else {
            ""
        };
        match result {
            Ok(requests) => {
                let _ = writeln!(
                    out,
                    "{} {:016x} {:016x} {requests} 1{lost}",
                    rec.calls,
                    rec.hash,
                    fnv(FNV0, &data)
                );
            }
            Err(e) => {
                match e {
                    QuickOrderError::Outside { .. } => at_position += 1,
                    QuickOrderError::ReversedRange { .. } => reversed += 1,
                }
                let _ = writeln!(out, "{} {:016x} outside", rec.calls, rec.hash);
            }
        }
    }
    eprintln!("outside: {at_position} at a position, {reversed} at a reversed range");
    out.flush().map_err(|e| e.to_string())
}

/// The merge order's best time over `repeats` sorts of `data` as `[u8; S]` elements (the typed
/// functions), with a plain comparator; and the sorted bytes' hash.
fn bench_typed<const S: usize>(data: &[u8], kind: Kind, hseed: u64, repeats: u64) -> (f64, u64) {
    let v: Vec<[u8; S]> = data
        .chunks(S)
        .map(|c| <[u8; S]>::try_from(c).unwrap())
        .collect();
    let mut best = f64::INFINITY;
    let mut check = 0;
    for _ in 0..repeats {
        let mut w = v.clone();
        let t = Instant::now();
        merge_order(&mut w, |a: &[u8; S], b: &[u8; S]| {
            compare(kind, hseed, a, b)
        });
        best = best.min(t.elapsed().as_secs_f64());
        check = fnv(FNV0, &w.concat());
    }
    (best, check)
}

/// `qsort-bench <s> <n> <mode> <seed> <keyrange> <repeats>`: the merge order's best time on one
/// case with a plain comparator (no counting or hashing), through the typed functions where the
/// size is one of the validation's (else the byte functions), then through the byte functions.
pub fn bench(args: &[String], out: &mut impl Write) -> Result<(), String> {
    if args.len() != 6 {
        return Err("usage: qsort-bench <s> <n> <mode> <seed> <keyrange> <repeats>".into());
    }
    let num = |i: usize| args[i].parse::<u64>().map_err(|e| e.to_string());
    let (s, n, seed, keyrange, repeats) = (
        num(0)? as usize,
        num(1)? as usize,
        num(3)?,
        num(4)?,
        num(5)?,
    );
    let kind = kind_of(&args[2]).ok_or("unknown mode")?;
    let hseed = seed ^ 0x5_DEEC_E66D;
    let data = elements(s, n, kind, seed, keyrange);
    let typed = match s {
        4 => Some(bench_typed::<4>(&data, kind, hseed, repeats)),
        8 => Some(bench_typed::<8>(&data, kind, hseed, repeats)),
        12 => Some(bench_typed::<12>(&data, kind, hseed, repeats)),
        16 => Some(bench_typed::<16>(&data, kind, hseed, repeats)),
        40 => Some(bench_typed::<40>(&data, kind, hseed, repeats)),
        104 => Some(bench_typed::<104>(&data, kind, hseed, repeats)),
        _ => None,
    };
    let mut best = f64::INFINITY;
    let mut check = 0;
    for _ in 0..repeats {
        let mut d = data.clone();
        let t = Instant::now();
        merge_order_bytes(&mut d, s, |a: &[u8], b: &[u8]| compare(kind, hseed, a, b));
        best = best.min(t.elapsed().as_secs_f64());
        check = fnv(FNV0, &d);
    }
    match typed {
        Some((t, c)) if c == check => {
            let _ = writeln!(out, "{t:.4} s typed, {best:.4} s bytes, {check:016x}");
        }
        Some(_) => return Err("the typed and the byte functions differ".into()),
        None => {
            let _ = writeln!(out, "{best:.4} s bytes, {check:016x}");
        }
    }
    out.flush().map_err(|e| e.to_string())
}
