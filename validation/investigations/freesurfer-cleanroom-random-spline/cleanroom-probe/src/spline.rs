// SPDX-License-Identifier: Apache-2.0
//! The cubic spline's commands, as `fs_spline_probe` reads and prints them (inferred from its
//! output, README):
//!
//! - `cases`: one case per line of standard input, `n ibcbeg ybcbeg ibcend ybcend m t[n] y[n]
//!   x[m]` (integers in decimal, floats as the 8 hex digits of their bits); prints `ypp` and the
//!   second derivatives, a `val` line per point (value, first and second derivative, `7fc00bad`
//!   where nothing is written), then `wrap` and `OpenSpline`'s second derivatives (with `ybcbeg`
//!   and `ybcend` as the end slopes) followed by `OpenSplint`'s value at every point. A fatal
//!   error prints FreeSurfer's message on standard output and ends with exit status 1.
//! - `d3`: one system per line, `n a[3n] b[n]`, `a` in band storage (column `j`: the
//!   super-diagonal entry above, the diagonal, the sub-diagonal entry below); prints `x` and the
//!   solution, or `null` when it is refused.
//! - `set1 <n> <ibcbeg> <ibcend> t... y...` (decimal numbers, end values 0): `ypp` and the
//!   second derivatives, or the fatal error's message and exit status 1.

use larmorx_core::UpstreamDefects;
use larmorx_core::cubic_spline::{
    EXIT_STATUS, evaluate, open_spline, open_splint, second_derivatives, solve_tridiagonal,
};
use larmorx_core::defects::Notices;
use std::io::{BufRead, Write};

/// The sentinel the probe puts in the derivative outputs before each evaluation.
const SENTINEL: u32 = 0x7fc0_0bad;

fn hex32(s: &str) -> Result<f32, String> {
    u32::from_str_radix(s, 16)
        .map(f32::from_bits)
        .map_err(|e| format!("bad float bits {s}: {e}"))
}

fn int(s: &str) -> Result<i64, String> {
    s.parse::<i64>().map_err(|e| format!("bad integer {s}: {e}"))
}

fn line(out: &mut impl Write, tag: &str, values: impl IntoIterator<Item = f32>) {
    let mut s = String::from(tag);
    for v in values {
        s.push_str(&format!(" {:08x}", v.to_bits()));
    }
    let _ = writeln!(out, "{s}");
}

fn fail(out: &mut impl Write, message: &str) -> Result<(), String> {
    let _ = write!(out, "{message}");
    let _ = out.flush();
    std::process::exit(i32::from(EXIT_STATUS));
}

pub(crate) fn run(args: &[String], out: &mut impl Write, mode: UpstreamDefects) -> Result<(), String> {
    match args[0].as_str() {
        "cases" => {
            for text in std::io::stdin().lock().lines() {
                let text = text.map_err(|e| e.to_string())?;
                let tok: Vec<&str> = text.split_whitespace().collect();
                if tok.len() < 6 {
                    continue;
                }
                let n = int(tok[0])? as usize;
                let ibcbeg = int(tok[1])? as i32;
                let ybcbeg = hex32(tok[2])?;
                let ibcend = int(tok[3])? as i32;
                let ybcend = hex32(tok[4])?;
                let m = int(tok[5])? as usize;
                let vals: Vec<f32> = tok[6..6 + 2 * n + m]
                    .iter()
                    .map(|s| hex32(s))
                    .collect::<Result<_, _>>()?;
                let (t, rest) = vals.split_at(n);
                let (y, xs) = rest.split_at(n);
                let mut notices = Notices::new(mode);
                let ypp = match second_derivatives(t, y, ibcbeg, ybcbeg, ibcend, ybcend, &mut notices) {
                    Ok(v) => v,
                    Err(e) => return fail(out, &e.message()),
                };
                line(out, "ypp", ypp.iter().copied());
                for &x in xs {
                    let e = evaluate(t, y, &ypp, x);
                    let s = f32::from_bits(SENTINEL);
                    line(out, "val", [e.value, e.first.unwrap_or(s), e.second.unwrap_or(s)]);
                }
                let w = match open_spline(t, y, ybcbeg, ybcend, &mut notices) {
                    Ok(v) => v,
                    Err(e) => return fail(out, &e.message()),
                };
                let values: Vec<f32> = xs.iter().map(|&x| open_splint(t, y, &w, x)).collect();
                line(out, "wrap", w.iter().copied().chain(values));
            }
        }
        "d3" => {
            for text in std::io::stdin().lock().lines() {
                let text = text.map_err(|e| e.to_string())?;
                let tok: Vec<&str> = text.split_whitespace().collect();
                if tok.is_empty() {
                    continue;
                }
                let n = int(tok[0])? as usize;
                let vals: Vec<f32> = tok[1..1 + 4 * n]
                    .iter()
                    .map(|s| hex32(s))
                    .collect::<Result<_, _>>()?;
                let (a, b) = vals.split_at(3 * n);
                let u: Vec<f32> = (0..n).map(|j| if j + 1 < n { a[3 * (j + 1)] } else { 0.0 }).collect();
                let d: Vec<f32> = (0..n).map(|j| a[3 * j + 1]).collect();
                let l: Vec<f32> = (0..n).map(|j| if j > 0 { a[3 * (j - 1) + 2] } else { 0.0 }).collect();
                match solve_tridiagonal(&l, &d, &u, b) {
                    Some(x) => line(out, "x", x),
                    None => {
                        let _ = writeln!(out, "null");
                    }
                }
            }
        }
        "set1" if args.len() >= 4 => {
            let n = int(&args[1])?;
            let ibcbeg = int(&args[2])? as i32;
            let ibcend = int(&args[3])? as i32;
            let nums: Vec<f32> = args[4..]
                .iter()
                .map(|s| crate::parse_f64(s).map(|v| v as f32))
                .collect::<Result<_, _>>()?;
            let n = usize::try_from(n).map_err(|_| "set1 takes n >= 0".to_string())?;
            if nums.len() < 2 * n {
                return Err(format!("set1 needs {} numbers", 2 * n));
            }
            let mut notices = Notices::new(mode);
            match second_derivatives(&nums[..n], &nums[n..2 * n], ibcbeg, 0.0, ibcend, 0.0, &mut notices) {
                Ok(v) => line(out, "ypp", v),
                Err(e) => return fail(out, &e.message()),
            }
        }
        _ => return Err("usage: cleanroom-probe cases|d3|set1 ...".to_string()),
    }
    let _ = out.flush();
    Ok(())
}
