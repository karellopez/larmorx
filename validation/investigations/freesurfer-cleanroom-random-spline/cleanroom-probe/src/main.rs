// SPDX-License-Identifier: Apache-2.0
//! larmorx's clean rooms of FreeSurfer 8.2.0's random numbers (`larmorx_core::ranlux`,
//! `specs/ranlux.md`) and cubic spline (`larmorx_core::cubic_spline`, `specs/burkardt-spline.md`)
//! behind the commands of the black-box probes `fs_rng_probe` and `fs_spline_probe`, so that the
//! two print the same lines for the same command (`compare.py` runs both).
//!
//! Random numbers: `raw <seed> <count>` (decimal integers), `dist <name> <seed> <count> [p1 [p2]]`
//! (each draw's bits in 16 hex digits), `mixed <seed> <count>` (one stream through a fixed
//! rotation of the draws). Before the command, `--reproduce` sets FS-100 and FS-101 to
//! FreeSurfer's behaviour (the default corrects them); notices go to standard error as larmorx's
//! command lines print them, errors too (exit status 1).
//!
//! Tools of the validation: `infer-mixed <seed>` (reads the probe's `mixed` lines on standard
//! input and names the draw that gives each, from the clean room's own draws: the rotation the
//! probe uses), `tan-list` (the `k` in [0, 2^24) where the C library's `tan` of `fl(pi*k*2^-24)`
//! differs from larmorx's correctly rounded `tan`: the system's `tan` run as a black box, through
//! Rust's `f64::tan`), `bench <seed> <count>` (the generator's speed, nothing printed).

mod glibc;
mod spline;

use larmorx_core::UpstreamDefects;
use larmorx_core::ranlux::Ranlux;
use std::io::{BufWriter, Write};
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// A seed as the probe reads it: an unsigned 64-bit integer, a negative one modulo 2^64.
fn parse_seed(s: &str) -> Result<u64, String> {
    if let Some(rest) = s.strip_prefix('-') {
        rest.parse::<u64>()
            .map(|v| v.wrapping_neg())
            .map_err(|e| format!("bad seed {s}: {e}"))
    } else {
        s.parse::<u64>().map_err(|e| format!("bad seed {s}: {e}"))
    }
}

/// A number as C's `strtod` reads the forms the probes are given: decimal, hexadecimal
/// (`0x1p-3`, exact mantissas only), `inf`, `infinity`, `nan`, `nan(<payload>)`, each signed.
pub(crate) fn parse_f64(s: &str) -> Result<f64, String> {
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
            if let Some(hex) = inner.strip_prefix("0x") {
                u64::from_str_radix(hex, 16).map_err(|e| format!("bad number {s}: {e}"))?
            } else {
                inner
                    .parse::<u64>()
                    .map_err(|e| format!("bad number {s}: {e}"))?
            }
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

/// A hexadecimal floating-point number (after `0x`) whose mantissa fits in 53 bits.
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
    let e = exp - 4 * frac.len() as i32;
    // m * 2^e, exact while the result is normal or zero.
    let v = m as f64 * 2f64.powi(e.clamp(-1022, 1023));
    let rest = e - e.clamp(-1022, 1023);
    Some(v * 2f64.powi(rest))
}

/// One draw of distribution `name` with parameters `p` (the probe's names).
fn draw(r: &mut Ranlux, name: &str, p: &[f64], n: u32) -> Result<f64, String> {
    let got = match name {
        "uni" => Ok(r.uniform()),
        "uni_pos" => r.uniform_pos(),
        "flat" => Ok(r.flat(p[0], p[1])),
        "gaussian" => r.gaussian(p[0]),
        "ugauss" => r.ugaussian(),
        "exponential" => r.exponential(p[0]),
        "gamma" => r.gamma(p[0], p[1]),
        "gamma_int" => r.gamma_int(n),
        "gamma_large" => r.gamma_large(p[0]),
        "gamma_frac" => r.gamma_frac(p[0]),
        "chisq" => r.chisq(p[0]),
        "fdist" => r.fdist(p[0], p[1]),
        "tdist" => r.tdist(p[0]),
        _ => return Err(format!("unknown distribution {name}")),
    };
    got.map_err(|e| e.to_string())
}

/// The parameters each distribution takes.
fn arity(name: &str) -> usize {
    match name {
        "uni" | "uni_pos" | "ugauss" => 0,
        "flat" | "gamma" | "fdist" => 2,
        _ => 1,
    }
}

/// The rotation of `mixed`, inferred from the probe's output by `infer-mixed` (README): 14 draws,
/// the t draw's degrees of freedom alternating between 1.5 and 7 from one round to the next.
/// (Positions 0 and 1 give the same bits as a uniform and a positive uniform, 10 as `chisq(5)` and
/// `gamma(2.5, 2)`: the names are the probe's order of its distributions.)
pub(crate) const MIXED: &[(&str, [f64; 2])] = &[
    ("uni", [0.0, 0.0]),
    ("uni_pos", [0.0, 0.0]),
    ("flat", [-3.5, 7.25]),
    ("gaussian", [2.5, 0.0]),
    ("ugauss", [0.0, 0.0]),
    ("gamma", [3.0, 2.0]),
    ("gamma", [0.4, 1.5]),
    ("gamma", [7.3, 0.5]),
    ("gamma", [25.0, 1.0]),
    ("gamma", [31.6, 1.0]),
    ("chisq", [5.0, 0.0]),
    ("fdist", [4.0, 9.0]),
    ("tdist", [1.5, 0.0]),
    ("exponential", [0.75, 0.0]),
];

/// The draw at position `i` of `mixed`.
fn mixed_draw(i: usize) -> (&'static str, [f64; 2]) {
    let (name, mut p) = MIXED[i % MIXED.len()];
    if name == "tdist" && (i / MIXED.len()) % 2 == 1 {
        p[0] = 7.0;
    }
    (name, p)
}

fn hex_line(out: &mut impl Write, v: f64) {
    let _ = writeln!(out, "{:016x}", v.to_bits());
}

fn print_notices(r: &mut Ranlux) {
    for n in r.take_notices() {
        eprintln!("larmorx: {}", n.line());
    }
}

/// The candidates `infer-mixed` tries at each position.
fn candidates() -> Vec<(String, Vec<f64>)> {
    let mut c: Vec<(String, Vec<f64>)> = vec![
        ("uni".into(), vec![]),
        ("uni_pos".into(), vec![]),
        ("ugauss".into(), vec![]),
    ];
    for (a, b) in [(0.0, 1.0), (-3.5, 7.25)] {
        c.push(("flat".into(), vec![a, b]));
    }
    for s in [1.0, 2.5] {
        c.push(("gaussian".into(), vec![s]));
    }
    for m in [1.0, 0.75] {
        c.push(("exponential".into(), vec![m]));
    }
    for a in [
        0.0, 0.4, 0.5, 1.0, 2.0, 2.5, 3.0, 7.3, 11.0, 11.5, 12.0, 12.5, 25.0, 31.6,
    ] {
        for b in [1.0, 2.0, 0.5, 1.5] {
            c.push(("gamma".into(), vec![a, b]));
        }
    }
    for nu in [1.0, 3.0, 5.0, 24.0, 25.0, 30.0] {
        c.push(("chisq".into(), vec![nu]));
    }
    for (a, b) in [(4.0, 9.0), (3.0, 25.0), (1.0, 1.0), (30.0, 50.0)] {
        c.push(("fdist".into(), vec![a, b]));
    }
    for nu in [0.5, 1.0, 1.5, 2.0, 2.25, 3.0, 7.0, 30.0] {
        c.push(("tdist".into(), vec![nu]));
    }
    c
}

/// Draws whose large-shape gamma met a tangent argument where the C library's `tan` differs from
/// the correctly rounded one (counted by the tangents below).
static TAN_HITS: AtomicU64 = AtomicU64::new(0);

/// larmorx's correctly rounded tangent, counting the arguments where the system's differs.
fn cr_tan_counting(x: f64) -> f64 {
    let c = larmorx_core::math::tan(x);
    if c.to_bits() != x.tan().to_bits() {
        TAN_HITS.fetch_add(1, Ordering::Relaxed);
    }
    c
}

/// The C library's tangent (a black box, through `f64::tan`), counting as above.
fn sys_tan_counting(x: f64) -> f64 {
    let s = x.tan();
    if s.to_bits() != larmorx_core::math::tan(x).to_bits() {
        TAN_HITS.fetch_add(1, Ordering::Relaxed);
    }
    s
}

/// The options before the command.
struct Options {
    mode: UpstreamDefects,
    /// The tangent the generators get.
    tan: fn(f64) -> f64,
    /// Where to write the indices of the draws that met a differing tangent.
    tan_hits: Option<String>,
}

impl Options {
    fn generator(&self, seed: u64) -> Ranlux {
        let mut r = Ranlux::new(seed, self.mode);
        r.set_tan(self.tan);
        r
    }
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
    let mut hits: Vec<usize> = Vec::new();
    let stdout = std::io::stdout();
    let mut out = BufWriter::with_capacity(1 << 16, stdout.lock());
    let usage = "usage: cleanroom-probe [--reproduce] [--system-tan] [--tan-hits FILE] raw|dist|mixed|infer-mixed|tan-list|bench|cases|d3|set1|glibc ...";
    match args.first().map(String::as_str) {
        Some("raw") if args.len() == 3 => {
            let mut r = opts.generator(parse_seed(&args[1])?);
            let count: usize = args[2].parse().map_err(|_| usage.to_string())?;
            let mut buf = [0u32; 4096];
            let mut left = count;
            while left > 0 {
                let take = left.min(buf.len());
                r.fill_u24(&mut buf[..take]);
                for v in &buf[..take] {
                    let _ = writeln!(out, "{v}");
                }
                left -= take;
            }
            let _ = out.flush();
            print_notices(&mut r);
        }
        Some("dist") if args.len() >= 4 => {
            let name = args[1].as_str();
            let mut r = opts.generator(parse_seed(&args[2])?);
            let count: usize = args[3].parse().map_err(|_| usage.to_string())?;
            let want = arity(name);
            let p: Vec<f64> = args[4..]
                .iter()
                .map(|s| parse_f64(s))
                .collect::<Result<_, _>>()?;
            if p.len() < want {
                return Err(format!("{name} takes {want} parameters"));
            }
            let n: u32 = if name == "gamma_int" {
                args[4]
                    .parse()
                    .map_err(|_| "bad integer shape".to_string())?
            } else {
                0
            };
            for i in 0..count {
                let before = TAN_HITS.load(Ordering::Relaxed);
                let got = draw(&mut r, name, &p, n);
                if TAN_HITS.load(Ordering::Relaxed) != before {
                    hits.push(i);
                }
                match got {
                    Ok(v) => hex_line(&mut out, v),
                    Err(e) => {
                        let _ = out.flush();
                        print_notices(&mut r);
                        return Err(e);
                    }
                }
            }
            let _ = out.flush();
            print_notices(&mut r);
        }
        Some("mixed") if args.len() == 3 => {
            let mut r = opts.generator(parse_seed(&args[1])?);
            let count: usize = args[2].parse().map_err(|_| usage.to_string())?;
            for i in 0..count {
                let (name, p) = mixed_draw(i);
                let before = TAN_HITS.load(Ordering::Relaxed);
                let got = draw(&mut r, name, &p, 0);
                if TAN_HITS.load(Ordering::Relaxed) != before {
                    hits.push(i);
                }
                match got {
                    Ok(v) => hex_line(&mut out, v),
                    Err(e) => {
                        let _ = out.flush();
                        return Err(e);
                    }
                }
            }
            let _ = out.flush();
            print_notices(&mut r);
        }
        Some("infer-mixed") if args.len() == 2 => {
            let mut r = opts.generator(parse_seed(&args[1])?);
            let lines: Vec<u64> = std::io::stdin()
                .lines()
                .map_while(Result::ok)
                .filter_map(|l| u64::from_str_radix(l.trim(), 16).ok())
                .collect();
            let cands = candidates();
            for (i, want) in lines.iter().enumerate() {
                let mut hits = Vec::new();
                let mut next = None;
                for (name, p) in &cands {
                    let mut t = r.clone();
                    if let Ok(v) = draw(&mut t, name, p, 0)
                        && v.to_bits() == *want
                    {
                        hits.push(format!("{name}{p:?}"));
                        if next.is_none() {
                            next = Some(t);
                        }
                    }
                }
                let _ = writeln!(out, "{i}: {}", hits.join(" | "));
                match next {
                    Some(t) => r = t,
                    None => {
                        let _ = writeln!(out, "no candidate gives line {i}: stop");
                        break;
                    }
                }
            }
            let _ = out.flush();
        }
        Some("tan-list") if args.len() == 1 => {
            let pi = f64::from_bits(0x4009_21fb_5444_2d18);
            let mut n = 0u64;
            let mut low = 0u64;
            for k in 0u32..(1 << 24) {
                let x = pi * (f64::from(k) * (1.0 / 16_777_216.0));
                let sys = x.tan();
                let cr = larmorx_core::math::tan(x);
                if sys.to_bits() != cr.to_bits() {
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
            let count: usize = args[2].parse().map_err(|_| usage.to_string())?;
            let t0 = Instant::now();
            let mut buf = [0u32; 4096];
            let mut sum = 0u64;
            let mut left = count;
            while left > 0 {
                let take = left.min(buf.len());
                r.fill_u24(&mut buf[..take]);
                sum += buf[..take].iter().map(|&v| u64::from(v)).sum::<u64>();
                left -= take;
            }
            let fill = t0.elapsed().as_secs_f64();
            let mut r = opts.generator(parse_seed(&args[1])?);
            let t1 = Instant::now();
            let mut sum2 = 0u64;
            for _ in 0..count {
                sum2 += u64::from(r.next_u24());
            }
            let single = t1.elapsed().as_secs_f64();
            assert_eq!(sum, sum2);
            let _ = writeln!(
                out,
                "{count} integers: fill_u24 {fill:.3} s, next_u24 {single:.3} s (sum {sum})"
            );
            let _ = out.flush();
        }
        Some("cases" | "d3" | "set1") => return spline::run(args, &mut out, opts.mode),
        Some("glibc") => return glibc::run(&args[1..], &mut out),
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
