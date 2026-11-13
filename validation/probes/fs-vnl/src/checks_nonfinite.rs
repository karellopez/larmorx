// SPDX-License-Identifier: Apache-2.0
//! EISPACK's `pythag`, `vnl_symmetric_eigensystem<float/double>` and LINPACK's `ssvdc` (as
//! `vnl_svd<float>` calls it) on infinities and NaNs, where VNL can loop for ever (ITK-115,
//! ITK-116). The port runs first; VNL runs in a child process (`fork`) with a time limit: where the
//! port says VNL never returns, the child must still be computing at the limit (its CPU time
//! measured, so a child the machine merely kept waiting is not taken for one that loops); where the
//! port returns, VNL must return the same values (a NaN compared as a NaN, as in `linalg`).

use std::ffi::{c_int, c_long};
use std::io::{Read, Write};
use std::os::fd::FromRawFd;

use larmorx_core::vnl_eigen;
use larmorx_core::vnl_svd_f32;
use larmorx_core::x87::cases::SplitMix64;

use crate::{Lib, Tally};

#[repr(C)]
struct Timeval {
    sec: i64,
    usec: i64,
}

#[repr(C)]
struct Rusage {
    utime: Timeval,
    stime: Timeval,
    rest: [i64; 14],
}

#[repr(C)]
struct PollFd {
    fd: c_int,
    events: i16,
    revents: i16,
}

unsafe extern "C" {
    fn fork() -> c_int;
    fn pipe(fds: *mut c_int) -> c_int;
    fn close(fd: c_int) -> c_int;
    fn poll(fds: *mut PollFd, n: u64, timeout_ms: c_int) -> c_int;
    fn kill(pid: c_int, sig: c_int) -> c_int;
    fn wait4(pid: c_int, status: *mut c_int, options: c_int, rusage: *mut Rusage) -> c_int;
    fn open(path: *const i8, flags: c_int, ...) -> c_int;
    fn dup2(old: c_int, new: c_int) -> c_int;
    fn _exit(status: c_int) -> !;
}

/// What a guarded call gave.
pub enum Guarded {
    /// The child finished and wrote these bytes.
    Done(Vec<u8>),
    /// The child was still computing at the limit, with this much CPU time (seconds).
    Spinning(f64),
}

/// Runs `f` in a child process; its standard error goes to `/dev/null` (VNL's messages are not
/// compared here). Returns what it wrote, or `Spinning` with its CPU time if it had not finished
/// after `ms` milliseconds (it is killed then).
pub fn guarded(ms: i32, f: impl FnOnce() -> Vec<u8>) -> Guarded {
    let mut fds = [0 as c_int; 2];
    // SAFETY: plain process and descriptor calls; the child runs `f`, writes and `_exit`s without
    // unwinding or flushing the parent's buffers.
    unsafe {
        assert_eq!(pipe(fds.as_mut_ptr()), 0, "pipe");
        let pid = fork();
        assert!(pid >= 0, "fork");
        if pid == 0 {
            close(fds[0]);
            let null = open(c"/dev/null".as_ptr().cast(), 1);
            if null >= 0 {
                dup2(null, 2);
            }
            let out = f();
            let mut pipe_w = std::fs::File::from_raw_fd(fds[1]);
            if pipe_w.write_all(&out).is_err() {
                _exit(3);
            }
            _exit(0);
        }
        close(fds[1]);
        let mut pipe_r = std::fs::File::from_raw_fd(fds[0]);
        let start = std::time::Instant::now();
        let mut got = Vec::new();
        let mut buf = [0u8; 65536];
        let mut finished = false;
        loop {
            let left = ms as i128 - start.elapsed().as_millis() as i128;
            if left <= 0 {
                break;
            }
            let mut p = PollFd {
                fd: fds[0],
                events: 1,
                revents: 0,
            };
            if poll(&mut p, 1, left as c_int) <= 0 {
                continue;
            }
            match pipe_r.read(&mut buf) {
                Ok(0) | Err(_) => {
                    finished = true;
                    break;
                }
                Ok(n) => got.extend_from_slice(&buf[..n]),
            }
        }
        drop(pipe_r);
        let mut status = 0;
        let mut ru = Rusage {
            utime: Timeval { sec: 0, usec: 0 },
            stime: Timeval { sec: 0, usec: 0 },
            rest: [0; 14],
        };
        if !finished {
            kill(pid, 9);
        }
        wait4(pid, &mut status, 0, &mut ru);
        if finished {
            Guarded::Done(got)
        } else {
            let t = |v: &Timeval| v.sec as f64 + v.usec as f64 * 1e-6;
            Guarded::Spinning(t(&ru.utime) + t(&ru.stime))
        }
    }
}

fn bytes_f64(v: &[f64]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn bytes_f32(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn from_f64(b: &[u8]) -> Vec<f64> {
    b.chunks_exact(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect()
}

fn from_f32(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect()
}

/// A number for these checks: an infinity, a NaN of either sign (with or without a payload), a
/// signed zero, or a finite value at a random scale.
fn special(rng: &mut SplitMix64) -> f64 {
    let u = (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
    match rng.below(10) {
        0 => f64::INFINITY,
        1 => f64::NEG_INFINITY,
        2 => f64::from_bits(0x7ff8_0000_0000_0000),
        3 => f64::from_bits(0xfff8_0000_0000_0000),
        4 => f64::from_bits(0x7ff8_0000_0000_0123),
        5 => 0.0,
        6 => -0.0,
        7 => (u - 0.5) * 1e300,
        _ => (u - 0.5) * 10f64.powi(rng.range(-5, 5) as i32),
    }
}

/// The time limit for a call the port says never returns (ms), and the CPU time (s) the child
/// must have used by then to count as looping; a call that returns takes microseconds.
const HANG_MS: i32 = 300;
const HANG_CPU: f64 = 0.15;
/// The time limit for a call the port says returns.
const DONE_MS: i32 = 20_000;
/// Children confirming hangs at once.
const PAR: usize = 6;

/// A call into VNL that the port says may never return.
enum Call {
    Pythag(f64, f64),
    Sym64(usize, Vec<f64>),
    Sym32(usize, Vec<f32>),
    Svd(usize, usize, Vec<f32>),
}

impl Call {
    fn run(&self, lib: &Lib) -> Vec<u8> {
        // SAFETY: the shim's signatures and array sizes.
        unsafe {
            match self {
                Call::Pythag(a, b) => bytes_f64(&[(lib.pythag)(*a, *b)]),
                Call::Sym64(n, m) => {
                    let (mut vec, mut val) = (vec![0.0f64; n * n], vec![0.0f64; *n]);
                    (lib.symeig_f64)(*n as c_int, m.as_ptr(), vec.as_mut_ptr(), val.as_mut_ptr());
                    val.extend_from_slice(&vec);
                    bytes_f64(&val)
                }
                Call::Sym32(n, m) => {
                    let (mut vec, mut val) = (vec![0.0f32; n * n], vec![0.0f32; *n]);
                    (lib.symeig_f32)(*n as c_int, m.as_ptr(), vec.as_mut_ptr(), val.as_mut_ptr());
                    val.extend_from_slice(&vec);
                    bytes_f32(&val)
                }
                Call::Svd(r, c, a) => {
                    let (mut uu, mut ww, mut vv) = (vec![0.0f32; r * c], vec![0.0f32; *c], vec![0.0f32; c * c]);
                    let info: c_long =
                        (lib.ssvdc_f32)(*r as c_int, *c as c_int, a.as_ptr(), uu.as_mut_ptr(), ww.as_mut_ptr(), vv.as_mut_ptr());
                    let mut all = vec![info as f32];
                    all.extend_from_slice(&uu);
                    all.extend_from_slice(&ww);
                    all.extend_from_slice(&vv);
                    bytes_f32(&all)
                }
            }
        }
    }

    fn what(&self) -> String {
        match self {
            Call::Pythag(a, b) => format!("pythag({a:e} {:#x}, {b:e} {:#x})", a.to_bits(), b.to_bits()),
            Call::Sym64(n, m) => format!("symeig f64 n={n} {m:?}"),
            Call::Sym32(n, m) => format!("symeig f32 n={n} {m:?}"),
            Call::Svd(r, c, a) => format!("ssvdc {r}x{c} {a:?}"),
        }
    }
}

/// Confirms that VNL is still computing each call at the limit (`PAR` children at a time; a
/// child the machine kept waiting is retried alone with longer limits). Returns how many were
/// confirmed; one that returned is a difference.
fn confirm_hangs(lib: &Lib, t: &mut Tally, calls: &[Call]) -> u64 {
    let mut confirmed = 0;
    for chunk in calls.chunks(PAR) {
        let mut kids = Vec::new();
        for call in chunk {
            let mut fds = [0 as c_int; 2];
            // SAFETY: as in `guarded`.
            unsafe {
                assert_eq!(pipe(fds.as_mut_ptr()), 0, "pipe");
                let pid = fork();
                assert!(pid >= 0, "fork");
                if pid == 0 {
                    close(fds[0]);
                    let null = open(c"/dev/null".as_ptr().cast(), 1);
                    if null >= 0 {
                        dup2(null, 2);
                    }
                    let out = call.run(lib);
                    let mut w = std::fs::File::from_raw_fd(fds[1]);
                    let _ = w.write_all(&out);
                    _exit(0);
                }
                close(fds[1]);
                kids.push((pid, std::fs::File::from_raw_fd(fds[0]), false));
            }
        }
        let start = std::time::Instant::now();
        let mut buf = [0u8; 65536];
        loop {
            let left = HANG_MS as i128 - start.elapsed().as_millis() as i128;
            if left <= 0 || kids.iter().all(|k| k.2) {
                break;
            }
            let mut pfds: Vec<PollFd> = kids
                .iter()
                .filter(|k| !k.2)
                .map(|k| PollFd {
                    fd: std::os::fd::AsRawFd::as_raw_fd(&k.1),
                    events: 1,
                    revents: 0,
                })
                .collect();
            // SAFETY: an array of live descriptors.
            if unsafe { poll(pfds.as_mut_ptr(), pfds.len() as u64, left as c_int) } <= 0 {
                continue;
            }
            for p in &pfds {
                if p.revents == 0 {
                    continue;
                }
                let k = kids.iter_mut().find(|k| std::os::fd::AsRawFd::as_raw_fd(&k.1) == p.fd).unwrap();
                match k.1.read(&mut buf) {
                    Ok(0) | Err(_) => k.2 = true,
                    Ok(_) => {}
                }
            }
        }
        for (call, (pid, file, finished)) in chunk.iter().zip(kids) {
            drop(file);
            let mut status = 0;
            let mut ru = Rusage {
                utime: Timeval { sec: 0, usec: 0 },
                stime: Timeval { sec: 0, usec: 0 },
                rest: [0; 14],
            };
            // SAFETY: our own child.
            unsafe {
                if !finished {
                    kill(pid, 9);
                }
                wait4(pid, &mut status, 0, &mut ru);
            }
            let cpu = ru.utime.sec as f64 + ru.utime.usec as f64 * 1e-6 + ru.stime.sec as f64 + ru.stime.usec as f64 * 1e-6;
            t.values += 1;
            if finished {
                t.differ += 1;
                if t.shown < 12 {
                    t.shown += 1;
                    println!("  DIFF [{}] {}: the port says VNL never returns, VNL returned", t.name, call.what());
                }
            } else if cpu >= HANG_CPU || confirm_hang(t, &|| call.what(), &|| call.run(lib)) {
                confirmed += 1;
            }
        }
    }
    confirmed
}

/// Confirms one hang alone, with longer limits while the child was kept waiting.
fn confirm_hang(t: &mut Tally, what: &dyn Fn() -> String, f: &dyn Fn() -> Vec<u8>) -> bool {
    for k in 2..=5 {
        match guarded(HANG_MS * k, f) {
            Guarded::Spinning(cpu) if cpu >= HANG_CPU * k as f64 => return true,
            Guarded::Spinning(_) => continue,
            Guarded::Done(_) => {
                t.differ += 1;
                if t.shown < 12 {
                    t.shown += 1;
                    println!("  DIFF [{}] {}: the port says VNL never returns, VNL returned", t.name, what());
                }
                return false;
            }
        }
    }
    t.notes.push(format!("{}: not confirmed (the machine kept the child waiting)", what()));
    false
}

/// Runs a call the port says returns; a VNL that does not is a difference.
fn expect_done(t: &mut Tally, what: &dyn Fn() -> String, f: &dyn Fn() -> Vec<u8>) -> Option<Vec<u8>> {
    match guarded(DONE_MS, f) {
        Guarded::Done(b) => Some(b),
        Guarded::Spinning(_) => {
            t.values += 1;
            t.differ += 1;
            if t.shown < 12 {
                t.shown += 1;
                println!("  DIFF [{}] {}: the port returns, VNL did not within {DONE_MS} ms", t.name, what());
            }
            None
        }
    }
}

/// The kind of a number for sampling `pythag`'s hangs: NaN by sign and payload, infinities,
/// zeros by sign, huge, other.
fn kind(x: f64) -> u8 {
    match x.to_bits() {
        0x7ff8_0000_0000_0000 => 0,
        0xfff8_0000_0000_0000 => 1,
        b if x.is_nan() && b >> 63 == 0 => 2,
        _ if x.is_nan() => 3,
        _ if x == f64::INFINITY => 4,
        _ if x == f64::NEG_INFINITY => 5,
        0 => 6,
        0x8000_0000_0000_0000 => 7,
        _ if x.abs() > 1e200 => 8,
        _ => 9,
    }
}

pub fn run(lib: &Lib, scale: u64, t: &mut Tally) {
    let mut rng = SplitMix64::new(0x0f1e_5eed);
    let mut hang_calls: [Vec<Call>; 3] = [Vec::new(), Vec::new(), Vec::new()];

    // pythag: every pair the port says returns, in one child; of those it says never return, up to
    // six of each pair of kinds.
    let (nan, inf) = (f64::NAN, f64::INFINITY);
    // The unit tests' pairs (vnl_eigen's pythag_keeps_vxls_nans) first.
    let mut pairs: Vec<(f64, f64)> = vec![
        (nan, 3.0),
        (-nan, -2.0),
        (nan, 0.0),
        (3.0, nan),
        (0.0, nan),
        (nan, nan),
        (inf, -inf),
        (nan, inf),
        (inf, 2.0),
        (-7.0, -inf),
        (inf, nan),
        (-0.0, -0.0),
        (-0.0, 0.0),
        (0.0, -0.0),
    ];
    pairs.extend((0..(20_000 * scale)).map(|_| (special(&mut rng), special(&mut rng))));
    let (ret, never): (Vec<_>, Vec<_>) = pairs.iter().partition(|(a, b)| vnl_eigen::pythag(*a, *b).is_some());
    t.cases += pairs.len() as u64;
    if let Some(theirs) = expect_done(t, &|| "pythag".to_owned(), &|| {
        // SAFETY: the shim's signature.
        bytes_f64(&ret.iter().map(|(a, b)| unsafe { (lib.pythag)(*a, *b) }).collect::<Vec<_>>())
    }) {
        let theirs = from_f64(&theirs);
        for ((a, b), th) in ret.iter().zip(&theirs) {
            let ours = vnl_eigen::pythag(*a, *b).unwrap();
            t.f64s(&|| format!("pythag({a:e} {:#x}, {b:e} {:#x})", a.to_bits(), b.to_bits()), &[ours], &[*th]);
        }
    }
    let mut per_kind = std::collections::HashMap::new();
    for (k, (a, b)) in never.iter().enumerate() {
        let c = per_kind.entry((kind(*a), kind(*b))).or_insert(0u32);
        // The unit tests' pairs always (they come first).
        if *c < 6 || k < 7 {
            *c += 1;
            hang_calls[0].push(Call::Pythag(*a, *b));
        }
    }
    let confirmed_p = confirm_hangs(lib, t, &hang_calls[0]);
    t.notes.push(format!(
        "pythag: {} pairs; {} return in the port (each compared with VNL); {} never return in the port, {} of them sampled ({} kinds of pair) and VNL's looping confirmed for {}",
        pairs.len(),
        ret.len(),
        never.len(),
        hang_calls[0].len(),
        per_kind.len(),
        confirmed_p
    ));

    // Symmetric eigensystems (double and float) and the float SVD of matrices with infinite and NaN
    // entries.
    let (mut sym_ret, mut svd_ret) = (0u64, 0u64);
    // The unit tests' matrices first (vnl_eigen's symmetric_eigensystems_where_vnl_never_returns,
    // vnl_svd_f32's ssvdc_stops_where_vnl_never_returns).
    let fixed_sym: [Vec<f64>; 3] = [vec![1.0, nan, nan, 2.0], vec![inf, 0.0, 0.0, 2.0], vec![4.0, 1.0, 1.0, 3.0]];
    let fixed_svd: [(usize, usize, Vec<f32>); 3] = [
        (3, 2, vec![-1.0, -1.0, -5.0, -1.0, 1.0, f32::INFINITY]),
        (3, 3, vec![1.0, -5.0, 5.0, 1.0, f32::NEG_INFINITY, -5.0, -4.0, 2.0, 5.0]),
        (3, 2, vec![3.0, 1.0, 1.0, 2.0, 0.5, -1.0]),
    ];
    let nfixed = fixed_sym.len() as u64;
    for case in 0..(2_000 * scale + nfixed) {
        t.cases += 1;
        let u = |rng: &mut SplitMix64| (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        let (n, m) = if case < nfixed {
            (2, fixed_sym[case as usize].clone())
        } else {
            let n = 1 + rng.below(6) as usize;
            let mut m = vec![0.0f64; n * n];
            for i in 0..n {
                for j in 0..=i {
                    let v = (u(&mut rng) - 0.5) * 10f64.powi(rng.range(-2, 3) as i32);
                    m[i * n + j] = v;
                    m[j * n + i] = v;
                }
            }
            for _ in 0..(1 + rng.below(3)) {
                let (i, j) = (rng.below(n as u64) as usize, rng.below(n as u64) as usize);
                let v = special(&mut rng);
                m[i * n + j] = v;
                if rng.below(3) != 0 {
                    m[j * n + i] = v;
                }
            }
            (n, m)
        };
        let w = |s: &str| format!("case {case} n={n} {s} {m:?}");
        let e = vnl_eigen::symmetric_eigensystem(&m, n);
        let call = Call::Sym64(n, m.clone());
        if e.never_returns() {
            hang_calls[1].push(call);
        } else if let Some(b) = expect_done(t, &|| w("symeig f64"), &|| call.run(lib)) {
            sym_ret += 1;
            let theirs = from_f64(&b);
            t.f64s(&|| w("symeig f64 D"), &e.values, &theirs[..n]);
            if e.ierr == 0 {
                t.f64s(&|| w("symeig f64 V"), &e.vectors, &theirs[n..]);
            }
        }
        let mf: Vec<f32> = m.iter().map(|&v| v as f32).collect();
        let ef = vnl_eigen::symmetric_eigensystem_f32_full(&mf, n);
        let call = Call::Sym32(n, mf.clone());
        if ef.never_returns() {
            hang_calls[1].push(call);
        } else if let Some(b) = expect_done(t, &|| w("symeig f32"), &|| call.run(lib)) {
            sym_ret += 1;
            let theirs = from_f32(&b);
            t.f32s(&|| w("symeig f32 D"), &ef.values, &theirs[..n]);
            if ef.ierr == 0 {
                t.f32s(&|| w("symeig f32 V"), &ef.vectors, &theirs[n..]);
            }
        }
        let (r, c, a) = if case < nfixed {
            fixed_svd[case as usize].clone()
        } else {
            let c = 1 + rng.below(5) as usize;
            let r = c + rng.below(3) as usize;
            let mut a: Vec<f32> =
                (0..r * c).map(|_| ((u(&mut rng) - 0.5) * 10f64.powi(rng.range(-2, 3) as i32)) as f32).collect();
            for _ in 0..rng.below(4) {
                let i = rng.below((r * c) as u64) as usize;
                a[i] = special(&mut rng) as f32;
            }
            (r, c, a)
        };
        let ws = |s: &str| format!("case {case} ssvdc {r}x{c} {s} {a:?}");
        let svd = vnl_svd_f32::vnl_svd_f32_full(&a, r, c);
        let call = Call::Svd(r, c, a.clone());
        if svd.never_returns() {
            hang_calls[2].push(call);
        } else if let Some(b) = expect_done(t, &|| ws(""), &|| call.run(lib)) {
            svd_ret += 1;
            let theirs = from_f32(&b);
            t.bits(&|| ws("info"), svd.info as u64, theirs[0] as u64);
            let (tu, rest) = theirs[1..].split_at(r * c);
            let (tw, tv) = rest.split_at(c);
            let cr = |cm: &[f32], rows: usize, cols: usize| {
                let mut out = vec![0.0f32; rows * cols];
                for i in 0..rows {
                    for j in 0..cols {
                        out[i * cols + j] = cm[i + rows * j];
                    }
                }
                out
            };
            t.f32s(&|| ws("U"), &cr(&svd.u, r, c), tu);
            t.f32s(&|| ws("W"), &svd.w, tw);
            t.f32s(&|| ws("V"), &cr(&svd.v, c, c), tv);
        }
    }
    let confirmed_s = confirm_hangs(lib, t, &hang_calls[1]);
    let confirmed_v = confirm_hangs(lib, t, &hang_calls[2]);
    t.notes.push(format!(
        "symmetric eigensystems: {sym_ret} returned in both (compared), {} never return in the port, VNL's looping confirmed for {confirmed_s}; ssvdc: {svd_ret} returned in both (compared), {} never return in the port, confirmed for {confirmed_v}",
        hang_calls[1].len(),
        hang_calls[2].len()
    ));
}
