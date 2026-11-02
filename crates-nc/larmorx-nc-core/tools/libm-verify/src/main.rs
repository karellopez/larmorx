// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret.
//! libm-verify: runs `glibc_reference` (this machine's glibc and x87 unit) and the replica's
//! ports on the same operands and reports every difference.
//!
//! Usage: `libm-verify <path to glibc_reference> [count]` (default 2,000,000 per family).
//! Meaningful on x86-64 Linux with glibc 2.35 on a CPU with FMA and AVX2 (the oracle machine).

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use larmorx_nc_core::{fdlibm, glibc_double as glibc_pow, glibc_float, x87};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    /// log-uniform in [lo, hi]
    fn log_uniform(&mut self, lo: f64, hi: f64) -> f64 {
        (lo.ln() + self.unit() * (hi.ln() - lo.ln())).exp()
    }
}

type Case = (&'static str, f64, f64);

fn cases(n: usize) -> Vec<Case> {
    let mut r = Rng(0x9e3779b97f4a7c15);
    let mut v: Vec<Case> = Vec::new();
    for _ in 0..n {
        // BET's calls: the radius (x^(1/3), x^(1/2)) and the threshold exponent f^0.275
        v.push(("pow", r.log_uniform(1.0, 1e12), 1.0 / 3.0));
        v.push(("pow", r.log_uniform(1.0, 1e9), 0.5));
        v.push(("pow", r.unit(), 0.275));
        // general operands: any finite double with any exponent
        let x = f64::from_bits(r.next() & 0x7fff_ffff_ffff_ffff);
        let y = (r.unit() - 0.5) * 8.0;
        if x.is_finite() {
            v.push(("pow", x, y));
        }
        // tanh: BET's arguments are F * (rinv - E), F about 9, and the full range
        v.push(("tanh", (r.unit() - 0.5) * 60.0, 0.0));
        v.push(("tanh", (r.unit() - 0.5) * 2.0, 0.0));
        let t = f64::from_bits(r.next());
        if t.is_finite() {
            v.push(("tanh", t, 0.0));
        }
        // exp: newimage's Gaussian kernels, -(j*j) / (2 sigma^2) with sigma a float, and the
        // whole range (underflow, the subnormal results, overflow)
        let sigma = r.log_uniform(1e-3, 300.0) as f32;
        let j = (r.next() % 400) as i32;
        v.push((
            "exp",
            f64::from(-(j * j)) / (2.0 * f64::from(sigma) * f64::from(sigma)),
            0.0,
        ));
        v.push(("exp", (r.unit() - 0.5) * 1500.0, 0.0));
        v.push(("exp", (r.unit() - 0.5) * 4.0, 0.0));
        let e = f64::from_bits(r.next());
        if e.is_finite() {
            v.push(("exp", e, 0.0));
        }
        // log: FAST's arguments, the log intensities log(x + 1.0) of float voxel values, the
        // Gaussians' log(sqrt(2 pi sigma^2)) of float variances (PI a float), arguments close
        // to 1 (Arm's second path) and the whole range (subnormals, negative numbers)
        let vox = r.log_uniform(1e-6, 1e6) as f32;
        v.push(("log", f64::from(vox) + 1.0, 0.0));
        let var = r.log_uniform(1e-9, 1e3) as f32;
        // FAST's `const float PI=3.14159265`, the float nearest to pi
        let pi = f64::from(std::f32::consts::PI);
        v.push(("log", (2.0 * pi * f64::from(var)).sqrt(), 0.0));
        v.push(("log", 1.0 + (r.unit() - 0.5) * 0.15, 0.0));
        let l = f64::from_bits(r.next());
        if !l.is_nan() {
            v.push(("log", l, 0.0));
        }
        // logf and expf as FSL's binaries bind them (logf@GLIBC_2.2.5, expf@GLIBC_2.2.5: the
        // SVID-compatible wrappers): FAST's variances, energies and log intensities, and every
        // float (negative numbers, zeros, infinities)
        let fv = r.log_uniform(1e-9, 1e3) as f32;
        v.push(("logf", f64::from(fv), 0.0));
        let fx = f32::from_bits(r.next() as u32);
        if !fx.is_nan() {
            v.push(("logf", f64::from(fx), 0.0));
            v.push(("expf", f64::from(fx), 0.0));
        }
        v.push(("expf", f64::from(((r.unit() - 0.9) * 130.0) as f32), 0.0));
        // x87 operations on doubles of NIfTI matrices' size
        let a = (r.unit() - 0.5) * 8.0;
        let b = (r.unit() - 0.5) * 8.0 + 1e-3;
        v.push(("lddiv", a, b));
        v.push(("ldadd", a, b * 1e-17));
        v.push(("ldsub", 1.0, a * a * 1e-3));
        v.push(("ldmulf", a, b));
        v.push(("ldmul", a, b));
    }
    v
}

fn ours(op: &str, a: f64, b: f64) -> u64 {
    match op {
        "pow" => glibc_pow::pow(a, b).to_bits(),
        "tanh" => fdlibm::tanh(a).to_bits(),
        "exp" => glibc_pow::exp(a).to_bits(),
        "log" => glibc_pow::log_compat(a).to_bits(),
        "logf" => u64::from(glibc_float::logf_compat(a as f32).to_bits()),
        "expf" => u64::from(glibc_float::expf(a as f32).to_bits()),
        "lddiv" => x87::div_to_f64(a, b).to_bits(),
        "ldadd" => x87::add_to_f64(a, b).to_bits(),
        "ldsub" => x87::sub_to_f64(a, b).to_bits(),
        "ldmulf" => u64::from(x87::mul_to_f32(a, b).to_bits()),
        "ldmul" => x87::mul_to_f64(a, b).to_bits(),
        _ => unreachable!(),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let reference = args
        .get(1)
        .expect("usage: libm-verify <glibc_reference> [count]");
    let n: usize = args.get(2).map_or(2_000_000, |s| s.parse().expect("count"));
    let v = cases(n);
    let mut child = Command::new(reference)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("cannot run glibc_reference");
    let mut stdin = child.stdin.take().unwrap();
    let input: Vec<u8> = v
        .iter()
        .flat_map(|(op, a, b)| {
            format!("{op} {:016x} {:016x}\n", a.to_bits(), b.to_bits()).into_bytes()
        })
        .collect();
    let writer = std::thread::spawn(move || {
        stdin.write_all(&input).unwrap();
    });
    let out = BufReader::new(child.stdout.take().unwrap());
    let mut counts: std::collections::BTreeMap<&str, (usize, usize)> = Default::default();
    let mut shown = 0;
    for (line, (op, a, b)) in out.lines().zip(&v) {
        let want = u64::from_str_radix(line.unwrap().trim(), 16).unwrap();
        let got = ours(op, *a, *b);
        let e = counts.entry(op).or_default();
        e.0 += 1;
        if got != want {
            e.1 += 1;
            if shown < 20 {
                println!("{op}({a:e}, {b:e}): glibc {want:016x}, port {got:016x}");
                shown += 1;
            }
        }
    }
    writer.join().unwrap();
    let _ = child.wait();
    let mut bad = 0;
    for (op, (total, differ)) in &counts {
        println!("{op}: {differ} of {total} differ");
        bad += differ;
    }
    std::process::exit(i32::from(bad != 0));
}
