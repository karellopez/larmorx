// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The replica's cubic spline (`larmorx_freesurfer_core::spline`, `fs.spline`) behind the commands
//! of the black-box probe `fs_spline_probe` (`validation/investigations/freesurfer-ranlux-probe/`,
//! FreeSurfer's own functions) and of the clean room's `cleanroom-probe`, so that the three print
//! the same lines for the same input:
//!
//! - `cases`: one case per line of standard input, `n ibcbeg ybcbeg ibcend ybcend m t[n] y[n] x[m]`
//!   (integers in decimal, floats as the 8 hex digits of their bits); prints `ypp` and the second
//!   derivatives, a `val` line per point (value, first and second derivative output, which start as
//!   `7fc00bad`), then `wrap` and `OpenSpline`'s second derivatives followed by `OpenSplint`'s values;
//! - `d3`: one system per line, `n a[3n] b[n]`; prints `x` and the solution, or `null`;
//! - `set1 <n> <ibcbeg> <ibcend> t... y...` (decimal, the end values 0): `ypp` and the second
//!   derivatives.
//!
//! A fatal error prints FreeSurfer's lines on standard output and ends with exit status 1.
//! `--reproduce` before the command sets FS-103 to FreeSurfer's behaviour.

use larmorx_core::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::spline::{
    d3_np_fs, open_spline, open_splint, spline_cubic_set, spline_cubic_value,
};
use std::io::{BufRead, Write};
use std::process::ExitCode;

const SENTINEL: u32 = 0x7fc0_0bad;

fn hexf(s: &str) -> Result<f32, String> {
    u32::from_str_radix(s, 16)
        .map(f32::from_bits)
        .map_err(|e| format!("bad float bits {s}: {e}"))
}

fn int(s: &str) -> Result<i32, String> {
    s.parse::<i32>()
        .map_err(|e| format!("bad integer {s}: {e}"))
}

fn line(out: &mut impl Write, tag: &str, values: impl IntoIterator<Item = f32>) {
    let mut s = String::from(tag);
    for v in values {
        s.push_str(&format!(" {:08x}", v.to_bits()));
    }
    let _ = writeln!(out, "{s}");
}

/// `exit(1)` after a fatal error (its lines are already on `out`).
fn exit(out: &mut impl Write, e: Fatal) -> ! {
    let _ = out.flush();
    std::process::exit(i32::from(e.status()));
}

fn run(args: &[String]) -> Result<(), String> {
    let (mode, args) = match args.first().map(String::as_str) {
        Some("--reproduce") => (UpstreamDefects::Reproduce, &args[1..]),
        _ => (UpstreamDefects::Fix, args),
    };
    let mut g = Globals::new(mode);
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    match args.first().map(String::as_str) {
        Some("cases") => {
            for text in std::io::stdin().lock().lines() {
                let text = text.map_err(|e| e.to_string())?;
                let tok: Vec<&str> = text.split_whitespace().collect();
                if tok.is_empty() {
                    continue;
                }
                let n = int(tok[0])?;
                let ibcbeg = int(tok[1])?;
                let ybcbeg = hexf(tok[2])?;
                let ibcend = int(tok[3])?;
                let ybcend = hexf(tok[4])?;
                let m = int(tok[5])? as usize;
                let nn = n.max(0) as usize;
                let vals: Vec<f32> = tok[6..6 + 2 * nn + m]
                    .iter()
                    .map(|s| hexf(s))
                    .collect::<Result<_, _>>()?;
                let (t, rest) = vals.split_at(nn);
                let (y, tv) = rest.split_at(nn);
                let ypp = match spline_cubic_set(
                    &mut g, &mut out, n, t, y, ibcbeg, ybcbeg, ibcend, ybcend,
                ) {
                    Ok(v) => v,
                    Err(e) => exit(&mut out, e),
                };
                line(&mut out, "ypp", ypp.iter().copied());
                for &x in tv {
                    let (mut yp, mut ypp2) = (f32::from_bits(SENTINEL), f32::from_bits(SENTINEL));
                    let v = spline_cubic_value(n, t, x, y, &ypp, &mut yp, &mut ypp2);
                    line(&mut out, "val", [v, yp, ypp2]);
                }
                let mut w = vec![0.0f32; nn];
                if let Err(e) = open_spline(&mut g, &mut out, t, y, n, ybcbeg, ybcend, &mut w) {
                    exit(&mut out, e);
                }
                let values: Vec<f32> = tv
                    .iter()
                    .map(|&x| {
                        let mut v = 0.0;
                        open_splint(t, y, &w, n, x, &mut v);
                        v
                    })
                    .collect();
                line(&mut out, "wrap", w.iter().copied().chain(values));
                let _ = out.flush();
            }
        }
        Some("d3") => {
            for text in std::io::stdin().lock().lines() {
                let text = text.map_err(|e| e.to_string())?;
                let tok: Vec<&str> = text.split_whitespace().collect();
                if tok.is_empty() {
                    continue;
                }
                let n = int(tok[0])?.max(0) as usize;
                let vals: Vec<f32> = tok[1..1 + 4 * n]
                    .iter()
                    .map(|s| hexf(s))
                    .collect::<Result<_, _>>()?;
                let (a, b) = vals.split_at(3 * n);
                let mut a = a.to_vec();
                match d3_np_fs(&mut a, b) {
                    Some(x) => line(&mut out, "x", x),
                    None => {
                        let _ = writeln!(out, "null");
                    }
                }
            }
        }
        Some("set1") if args.len() >= 4 => {
            let n = int(&args[1])?;
            let ibcbeg = int(&args[2])?;
            let ibcend = int(&args[3])?;
            let nn = n.max(0) as usize;
            // `strtof` of each number
            let nums: Vec<f32> = args[4..]
                .iter()
                .map(|s| s.parse::<f32>().map_err(|e| format!("bad number {s}: {e}")))
                .collect::<Result<_, _>>()?;
            if nums.len() < 2 * nn {
                return Err(format!("set1 needs {} numbers", 2 * nn));
            }
            match spline_cubic_set(
                &mut g,
                &mut out,
                n,
                &nums[..nn],
                &nums[nn..2 * nn],
                ibcbeg,
                0.0,
                ibcend,
                0.0,
            ) {
                Ok(v) => line(&mut out, "ypp", v),
                Err(e) => exit(&mut out, e),
            }
        }
        _ => {
            return Err(
                "usage: fs_spline_replica_probe [--reproduce] cases|d3|set1 ...".to_string(),
            );
        }
    }
    let _ = out.flush();
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(2)
        }
    }
}
