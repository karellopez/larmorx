// SPDX-License-Identifier: GPL-3.0-or-later
// Part of larmorx-gpl (GNU GPL version 3 or later; see crates-gpl/LICENSE). Copyright 2026 Karel
// Lopez Vilaret.
//! The replica (`larmorx_gpl_gsl`) behind the commands of the black-box probe `fs_rng_probe`
//! (`validation/investigations/freesurfer-ranlux-probe/`, FreeSurfer's own functions) and of the
//! clean room's `cleanroom-probe` (`validation/investigations/freesurfer-cleanroom-random-spline/`),
//! so that the three print the same lines for the same command:
//!
//! - `raw <seed> <count>`: the generator's integers, in decimal;
//! - `dist <name> <seed> <count> [p1 [p2]]`: draws, each as the 16 hex digits of its bits (names:
//!   `uni uni_pos flat gaussian ugauss gamma gamma_int gamma_large gamma_frac chisq fdist tdist
//!   exponential`);
//! - `mixed <seed> <count>`: one stream through the probe's fixed rotation of every draw;
//! - `crafted <name> <a> <b> [p1 [p2]]`: one draw from the state the probe writes (every value 0
//!   but the two read next, `a` and `b`; the carry 0, the counters at their start);
//! - `tan-list`: the `k` in [0, 2²⁴) where the C library's `tan` of `fl(π·k·2⁻²⁴)` differs from
//!   larmorx's correctly rounded one (the system's `tan` through Rust's `f64::tan`);
//! - `bench <seed> <count>`: the generator's speed, nothing printed.
//!
//! Before the command: `--reproduce` (FS-100 and FS-101 as FreeSurfer; the default corrects them),
//! `--system-tan` (the large-shape gamma draw takes the C library's `tan`, for validation),
//! `--tan-hits FILE` (the indices of the draws that met a tangent argument where the two differ).
//! Notices go to standard error as larmorx's command lines print them, errors too (exit status 1).
//! Seeds are unsigned 64-bit integers, a negative one taken modulo 2⁶⁴ (as `fs_rng_probe`).

use larmorx_core::UpstreamDefects;
use larmorx_gpl_gsl::{DrawError, ScRng};
use std::io::{BufWriter, Write};
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

fn parse_seed(s: &str) -> Result<u64, String> {
    if let Some(rest) = s.strip_prefix('-') {
        rest.parse::<u64>()
            .map(u64::wrapping_neg)
            .map_err(|e| format!("bad seed {s}: {e}"))
    } else {
        s.parse::<u64>().map_err(|e| format!("bad seed {s}: {e}"))
    }
}

/// A hexadecimal number's digits after `0x` (`h.hhhp±d`), when its mantissa fits in 53 bits.
fn parse_hex(s: &str) -> Option<f64> {
    let (mant, exp) = match s.split_once('p') {
        Some((m, e)) => (m, e.parse::<i32>().ok()?),
        None => (s, 0),
    };
    let (int, frac) = mant.split_once('.').unwrap_or((mant, ""));
    let digits = format!("{int}{frac}");
    let m = u64::from_str_radix(if digits.is_empty() { "0" } else { &digits }, 16).ok()?;
    if m >= 1 << 53 {
        return None;
    }
    let mut e = exp - 4 * frac.len() as i32;
    let mut v = m as f64;
    while e > 0 {
        let step = e.min(1000);
        v *= 2f64.powi(step);
        e -= step;
    }
    while e < 0 {
        let step = (-e).min(1000);
        v /= 2f64.powi(step);
        e += step;
    }
    Some(v)
}

/// A number in the forms C's `strtod` reads in the probe's commands: decimal, `0x` hexadecimal,
/// `inf`, `infinity`, `nan`, `nan(<payload>)` (decimal or `0x`), each with a sign.
fn parse_f64(s: &str) -> Result<f64, String> {
    let (neg, body) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let lower = body.to_ascii_lowercase();
    let v = if let Some(rest) = lower.strip_prefix("nan") {
        let payload = if rest.is_empty() {
            0
        } else {
            let inner = rest
                .strip_prefix('(')
                .and_then(|r| r.strip_suffix(')'))
                .ok_or_else(|| format!("bad number {s}"))?;
            match inner.strip_prefix("0x") {
                Some(hex) => u64::from_str_radix(hex, 16),
                None => inner.parse::<u64>(),
            }
            .map_err(|e| format!("bad number {s}: {e}"))?
        };
        f64::from_bits(0x7ff8_0000_0000_0000 | (payload & 0x0007_ffff_ffff_ffff))
    } else if lower == "inf" || lower == "infinity" {
        f64::INFINITY
    } else if let Some(hex) = lower.strip_prefix("0x") {
        parse_hex(hex).ok_or_else(|| format!("bad number {s}"))?
    } else {
        body.parse::<f64>()
            .map_err(|e| format!("bad number {s}: {e}"))?
    };
    Ok(if neg { -v } else { v })
}

/// One draw of `name` (the probe's names) with parameters `p`.
fn draw(r: &mut ScRng, name: &str, p: [f64; 2]) -> Result<f64, String> {
    let got: Result<f64, DrawError> = match name {
        "uni" => Ok(r.uni()),
        "uni_pos" => r.uni_pos(),
        "flat" => Ok(r.ran_flat(p[0], p[1])),
        "gaussian" => r.ran_gaussian(p[0]),
        "ugauss" => r.ugauss(),
        "gamma" => r.ran_gamma(p[0], p[1]),
        // `(unsigned int)p1`, as the probe converts it (the cases are integers in range)
        "gamma_int" => r.gamma_int(p[0] as u32),
        "gamma_large" => r.gamma_large(p[0]),
        "gamma_frac" => r.gamma_frac(p[0]),
        "chisq" => r.ran_chisq(p[0]),
        "fdist" => r.ran_fdist(p[0], p[1]),
        "tdist" => r.ran_tdist(p[0]),
        "exponential" => r.ran_exponential(p[0]),
        _ => return Err(format!("unknown distribution {name}")),
    };
    got.map_err(|e| e.to_string())
}

/// `fs_rng_probe mixed`'s draw `k` (its source's rotation, `specs/ranlux.md` §10.2).
fn mixed_draw(r: &mut ScRng, k: usize) -> Result<f64, String> {
    let d = |name: &str, a: f64, b: f64| (name.to_string(), [a, b]);
    let (name, p) = match k % 14 {
        0 => d("uni", 0.0, 0.0),
        1 => d("uni_pos", 0.0, 0.0),
        2 => d("flat", -3.5, 7.25),
        3 => d("gaussian", 2.5, 0.0),
        4 => d("ugauss", 0.0, 0.0),
        5 => d("gamma", 3.0, 2.0),
        6 => d("gamma", 0.4, 1.5),
        7 => d("gamma", 7.3, 0.5),
        8 => d("gamma", 25.0, 1.0),
        9 => d("gamma", 31.6, 1.0),
        10 => d("chisq", 5.0, 0.0),
        11 => d("fdist", 4.0, 9.0),
        12 => d("tdist", if k % 28 == 12 { 1.5 } else { 7.0 }, 0.0),
        _ => d("exponential", 0.75, 0.0),
    };
    draw(r, &name, p)
}

/// Draws whose large-shape gamma met a tangent argument where the C library's `tan` differs from
/// the correctly rounded one.
static TAN_HITS: AtomicU64 = AtomicU64::new(0);

fn cr_tan_counting(x: f64) -> f64 {
    let c = larmorx_core::math::tan(x);
    if c.to_bits() != x.tan().to_bits() {
        TAN_HITS.fetch_add(1, Ordering::Relaxed);
    }
    c
}

fn sys_tan_counting(x: f64) -> f64 {
    let s = x.tan();
    if s.to_bits() != larmorx_core::math::tan(x).to_bits() {
        TAN_HITS.fetch_add(1, Ordering::Relaxed);
    }
    s
}

struct Options {
    mode: UpstreamDefects,
    tan: fn(f64) -> f64,
    tan_hits: Option<String>,
}

impl Options {
    fn generator(&self, seed: u64) -> ScRng {
        let mut r = ScRng::new(seed, self.mode);
        r.set_tan(self.tan);
        r
    }
}

fn print_notices(r: &mut ScRng) {
    for n in r.take_notices() {
        eprintln!("larmorx: {}", n.line());
    }
}

/// One draw's line, or its error (after the notices so far when `notices`, as `cleanroom-probe`).
fn emit(
    r: &mut ScRng,
    got: Result<f64, String>,
    out: &mut impl Write,
    notices: bool,
) -> Result<(), String> {
    match got {
        Ok(v) => {
            let _ = writeln!(out, "{:016x}", v.to_bits());
            Ok(())
        }
        Err(e) => {
            let _ = out.flush();
            if notices {
                print_notices(r);
            }
            Err(e)
        }
    }
}

fn params(args: &[String]) -> Result<[f64; 2], String> {
    let mut p = [0.0; 2];
    for (slot, s) in p.iter_mut().zip(args) {
        *slot = parse_f64(s)?;
    }
    Ok(p)
}

fn run(args: &[String]) -> Result<(), String> {
    let mut args = args;
    let mut opts = Options {
        mode: UpstreamDefects::Fix,
        tan: cr_tan_counting,
        tan_hits: None,
    };
    loop {
        match args.first().map(String::as_str) {
            Some("--reproduce") => opts.mode = UpstreamDefects::Reproduce,
            Some("--system-tan") => opts.tan = sys_tan_counting,
            Some("--tan-hits") if args.len() > 1 => {
                opts.tan_hits = Some(args[1].clone());
                args = &args[1..];
            }
            _ => break,
        }
        args = &args[1..];
    }
    let usage = "usage: gsl_probe [--reproduce] [--system-tan] [--tan-hits FILE] \
                 raw|dist|mixed|crafted|tan-list|bench ...";
    let count = |s: &String| s.parse::<usize>().map_err(|_| usage.to_string());
    let stdout = std::io::stdout();
    let mut out = BufWriter::with_capacity(1 << 16, stdout.lock());
    let mut hits: Vec<usize> = Vec::new();
    match args.first().map(String::as_str) {
        Some("raw") if args.len() == 3 => {
            let mut r = opts.generator(parse_seed(&args[1])?);
            for _ in 0..count(&args[2])? {
                let _ = writeln!(out, "{}", r.get());
            }
            let _ = out.flush();
            print_notices(&mut r);
        }
        Some("dist") if args.len() >= 4 => {
            let name = args[1].as_str();
            let mut r = opts.generator(parse_seed(&args[2])?);
            let p = params(&args[4..])?;
            for k in 0..count(&args[3])? {
                let before = TAN_HITS.load(Ordering::Relaxed);
                let got = draw(&mut r, name, p);
                if TAN_HITS.load(Ordering::Relaxed) != before {
                    hits.push(k);
                }
                emit(&mut r, got, &mut out, true)?;
            }
            let _ = out.flush();
            print_notices(&mut r);
        }
        Some("mixed") if args.len() == 3 => {
            let mut r = opts.generator(parse_seed(&args[1])?);
            for k in 0..count(&args[2])? {
                let before = TAN_HITS.load(Ordering::Relaxed);
                let got = mixed_draw(&mut r, k);
                if TAN_HITS.load(Ordering::Relaxed) != before {
                    hits.push(k);
                }
                emit(&mut r, got, &mut out, false)?;
            }
            let _ = out.flush();
            print_notices(&mut r);
        }
        Some("crafted") if args.len() >= 4 => {
            // the probe: sc_rng_set(r, 1), then u[] = 0 but u[9] = a, u[8] = b (24 bits each),
            // carry 0, n 0, i 23, j 9
            let mut r = opts.generator(1);
            let a = args[2].parse::<u64>().map_err(|_| usage.to_string())?;
            let b = args[3].parse::<u64>().map_err(|_| usage.to_string())?;
            let st = r.status_mut();
            st.u = [0; 24];
            st.carry = 0;
            st.n = 0;
            st.i = 23;
            st.j = 9;
            st.u[9] = a & 0xff_ffff;
            st.u[8] = b & 0xff_ffff;
            let p = params(&args[4..])?;
            let got = draw(&mut r, &args[1], p);
            emit(&mut r, got, &mut out, true)?;
            let _ = out.flush();
            print_notices(&mut r);
        }
        Some("tan-list") if args.len() == 1 => {
            let mut n = 0u64;
            let mut low = 0u64;
            for k in 0u32..(1 << 24) {
                let x = std::f64::consts::PI * (f64::from(k) / 16_777_216.0);
                if x.tan().to_bits() != larmorx_core::math::tan(x).to_bits() {
                    let _ = writeln!(out, "{k}");
                    n += 1;
                    low += u64::from(k < (1 << 23));
                }
            }
            let _ = out.flush();
            eprintln!("{n} of 16777216 arguments differ ({low} with k < 2^23)");
        }
        Some("bench") if args.len() == 3 => {
            let mut r = opts.generator(parse_seed(&args[1])?);
            let n = count(&args[2])?;
            let t0 = Instant::now();
            let mut sum = 0u64;
            for _ in 0..n {
                sum = sum.wrapping_add(r.get());
            }
            let dt = t0.elapsed().as_secs_f64();
            let _ = writeln!(out, "{n} integers: sc_rng_get {dt:.3} s (sum {sum})");
            let _ = out.flush();
        }
        _ => return Err(usage.to_string()),
    }
    if let Some(path) = &opts.tan_hits {
        let text: String = hits.iter().map(|i| format!("{i}\n")).collect();
        std::fs::write(path, text).map_err(|e| format!("{path}: {e}"))?;
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
