// SPDX-License-Identifier: Apache-2.0
//! What `larmorx fsl topup` reports while it estimates (`specs/topup.md` §2.2, §5): `-v`'s cost
//! lines, `--debug`'s files, `--trace`'s stages and `--profile`'s durations.
//!
//! `--debug`'s files have FSL's names (observed, black-box runs: `TopupDebug<what>level<LL>_
//! iter<II>_attempt<AA>`, the evaluations numbered by the iteration and the attempt within it,
//! the gradients and Hessians with attempt 0; the scaled conjugate gradients number by a
//! counter of the gradients, findings 9) and the original's own values of those quantities:
//! level 1 writes each evaluation's field (Hz), corrected volumes, Jacobians and movement
//! parameters and each gradient; level 2 adds the mask, the masked differences from the mean,
//! the gradient with the regulariser and the Hessian's voxel weights; level 3 the Hessian
//! itself (one `row  column  value` line per nonzero, by columns, the last diagonal entry
//! always). `--trace` prints the original's own stages (its internals are not FSL's), each as
//! an `Entering <stage>` / `Leaving <stage>` pair; `--profile` writes FSL's two kinds of
//! entries, `Entry: Begins topup level <l> Duration: <seconds>` per level and `Entry: Begins
//! topup run Duration: <seconds>` last.

use std::io::Write;
use std::time::Instant;

use larmorx_core::defects::UpstreamDefects;
use larmorx_io::nifti::NiftiHeader;

use super::Settings;
use super::fmt::g;
use crate::bet::image::{self, Output, Values};
use crate::topup::model::{Eval, Level};
use crate::topup::stencil::Hessian;
use crate::topup::{self, Progress};

/// The suffix of `--profile`'s file after `--out`.
pub const PROFILE_SUFFIX: &str = ".topup_profile_topup.cpp_main.txt";

/// `%.10g` followed by two blanks (FSL's text matrices).
pub fn g10(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    if !v.is_finite() {
        return if v.is_nan() {
            "nan".into()
        } else if v > 0.0 {
            "inf".into()
        } else {
            "-inf".into()
        };
    }
    let sci = format!("{:.9e}", v);
    let (mantissa, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let x: i32 = exp.parse().unwrap_or(0);
    let strip = |s: &str| -> String {
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_owned()
        } else {
            s.to_owned()
        }
    };
    if !(-4..10).contains(&x) {
        let sign = if x < 0 { '-' } else { '+' };
        format!("{}e{sign}{:02}", strip(mantissa), x.abs())
    } else {
        let decimals = (9 - x).max(0) as usize;
        strip(&format!("{v:.decimals$}"))
    }
}

/// `%.8g` (the Hessian's values).
fn g8(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    let sci = format!("{:.7e}", v);
    let (mantissa, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let x: i32 = exp.parse().unwrap_or(0);
    let strip = |s: &str| -> String {
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_owned()
        } else {
            s.to_owned()
        }
    };
    if !(-4..8).contains(&x) {
        let sign = if x < 0 { '-' } else { '+' };
        format!("{}e{sign}{:02}", strip(mantissa), x.abs())
    } else {
        let decimals = (7 - x).max(0) as usize;
        strip(&format!("{v:.decimals$}"))
    }
}

/// Where `--debug`'s images go.
pub struct DebugOut<'a> {
    pub settings: &'a Settings,
    pub header: &'a NiftiHeader,
    pub flipped: bool,
    /// `FSLOUTPUTTYPE`, when FSL knows it (else no debug image is written).
    pub output_type: Option<String>,
    pub mode: UpstreamDefects,
    pub n_threads: usize,
    pub level: u8,
}

impl DebugOut<'_> {
    fn image(&self, stem: &str, shape: [usize; 3], voxel: [f64; 3], volumes: usize, v: Values) {
        let Some(ot) = &self.output_type else { return };
        let stem = self.settings.name(stem);
        let Some((name, version)) = image::output_file(&stem, ot) else {
            return;
        };
        let mut h = self.header.clone();
        h.qform_code = 0;
        h.sform_code = 0;
        h.intent_code = 0;
        h.intent_p1 = 0.0;
        h.intent_p2 = 0.0;
        h.intent_p3 = 0.0;
        h.pixdim = [1.0, voxel[0], voxel[1], voxel[2], 1.0, 1.0, 1.0, 1.0];
        let o = Output {
            template: &h,
            flipped: self.flipped,
            shape,
            volumes,
            cal: None,
            extensions: false,
            defects: self.mode,
            n_threads: self.n_threads,
            descrip: super::topup::DESCRIP,
            rewrite_quaternion: false,
        };
        let _ = image::write(&name, version, &o, v);
    }

    fn text(&self, name: &str, text: &str) {
        let _ = std::fs::write(self.settings.path(name), text);
    }
}

/// The run's reporter: `-v`, `--debug`, `--trace` and `--profile`.
pub struct Printer<'a> {
    pub out: &'a mut Vec<u8>,
    pub verbose: bool,
    pub trace: bool,
    pub debug: Option<DebugOut<'a>>,
    /// The levels' durations (`--profile`).
    pub started: Option<Instant>,
    pub durations: Vec<f64>,
}

impl Printer<'_> {
    /// One stage of `--trace`.
    pub fn stage(&mut self, name: &str, entering: bool) {
        if self.trace {
            let what = if entering { "Entering" } else { "Leaving" };
            let _ = writeln!(self.out, "{what} larmorx::topup::{name}");
        }
    }

    fn tag(index: usize, iter: usize, attempt: usize) -> String {
        format!(
            "level{:02}_iter{:02}_attempt{:02}",
            index + 1,
            iter,
            attempt
        )
    }
}

impl Progress for Printer<'_> {
    fn evaluated(&mut self, index: usize, level: &Level, e: &Eval, iter: usize, attempt: usize) {
        self.stage("evaluate", true);
        if self.verbose {
            let reg = e.cost - e.ssd;
            let _ = writeln!(
                self.out,
                "SSD = {}\tn = {}\tReg = {}\tCost = {}",
                g(e.ssd),
                e.n_mask,
                g(reg),
                g(e.cost)
            );
        }
        if let Some(d) = &self.debug {
            let tag = Self::tag(index, iter, attempt);
            let nt = level.nt;
            let f32s = |v: &[f64]| v.iter().map(|&x| x as f32).collect::<Vec<f32>>();
            d.image(
                &format!("TopupDebugField{tag}"),
                nt,
                level.vt,
                1,
                Values::F32(f32s(&e.h)),
            );
            let m = e.u.len();
            let cat = |vs: &[Vec<f64>]| vs.iter().flat_map(|v| f32s(v)).collect::<Vec<f32>>();
            d.image(
                &format!("TopupDebugUnwarped{tag}"),
                nt,
                [1.0; 3],
                m,
                Values::F32(cat(&e.u)),
            );
            d.image(
                &format!("TopupDebugJacobians{tag}"),
                nt,
                [1.0; 3],
                m,
                Values::F32(cat(&e.jac)),
            );
            let text: String = e
                .movements()
                .iter()
                .map(|p| {
                    p.iter()
                        .map(|&v| format!("{}  ", g10(v)))
                        .collect::<String>()
                        + "\n"
                })
                .collect();
            d.text(&format!("TopupDebugMovementParameters{tag}.txt"), &text);
            if d.level >= 2 {
                let mask: Vec<u8> = e.mask.iter().map(|&b| u8::from(b)).collect();
                d.image(
                    &format!("TopupDebugMask{tag}"),
                    nt,
                    level.vt,
                    1,
                    Values::U8(mask),
                );
                let nvox = e.h.len();
                let diff: Vec<f32> = (0..m)
                    .flat_map(|i| {
                        (0..nvox).map(move |v| {
                            if e.mask[v] {
                                let mean = e.u.iter().map(|u| u[v]).sum::<f64>() / m as f64;
                                (e.u[i][v] - mean) as f32
                            } else {
                                0.0
                            }
                        })
                    })
                    .collect();
                d.image(
                    &format!("TopupDebugMaskedDiff{tag}"),
                    nt,
                    [1.0; 3],
                    m,
                    Values::F32(diff),
                );
            }
        }
        self.stage("evaluate", false);
    }

    fn next_level(&mut self, _index: usize) {
        if self.verbose {
            let _ = writeln!(self.out, "***Going to next resolution level***");
        }
    }

    fn gradient(&mut self, index: usize, level: &Level, e: &Eval, gr: &[f64], iter: usize) {
        self.stage("gradient", true);
        if let Some(d) = &self.debug {
            let tag = Self::tag(index, iter, 0);
            let lam = e.lambda_eff(level);
            let nc = level.n_coef();
            let lines = |v: &mut dyn Iterator<Item = f64>| -> String {
                v.map(|x| format!("{}  \n", g10(x))).collect()
            };
            // without the regulariser: the regulariser's part `2 λ' R c` taken out
            let mut noreg = gr.iter().enumerate().map(|(k, &x)| {
                if k < nc && level.lambda != 0.0 {
                    x - 2.0 * lam * e.rc[k]
                } else {
                    x
                }
            });
            d.text(&format!("TopupDebugGradient_{tag}.txt"), &lines(&mut noreg));
            if d.level >= 2 {
                d.text(
                    &format!("TopupDebugGradientWithReg_{tag}.txt"),
                    &lines(&mut gr.iter().copied()),
                );
            }
        }
        self.stage("gradient", false);
    }

    fn hessian(&mut self, index: usize, level: &Level, e: &Eval, h: &Hessian, iter: usize) {
        self.stage("hessian", true);
        if let Some(d) = &self.debug
            && d.level >= 2
        {
            let tag = Self::tag(index, iter, 0);
            let w = level.voxel_weights(e);
            let nvox = e.h.len();
            let vols: Vec<f32> = w.iter().flat_map(|v| v.iter().map(|&x| x as f32)).collect();
            debug_assert_eq!(vols.len(), 6 * nvox);
            d.image(
                &format!("TopupDebugABC{tag}"),
                level.nt,
                level.vt,
                6,
                Values::F32(vols),
            );
            if d.level >= 3 {
                d.text(&format!("TopupDebugHess{tag}.txt"), &hessian_text(h));
            }
        }
        self.stage("hessian", false);
    }

    fn level_started(&mut self, index: usize) {
        self.stage(&format!("level {}", index + 1), true);
        if self.started.is_some() {
            self.durations.push(-now_seconds());
        }
    }

    fn level_finished(&mut self, index: usize) {
        if self.started.is_some()
            && let Some(d) = self.durations.last_mut()
        {
            *d += now_seconds();
        }
        self.stage(&format!("level {}", index + 1), false);
    }
}

fn now_seconds() -> f64 {
    use std::sync::OnceLock;
    static T0: OnceLock<Instant> = OnceLock::new();
    T0.get_or_init(Instant::now).elapsed().as_secs_f64()
}

/// The Hessian as `row  column  value` lines (1-based), by columns, the nonzeros and the last
/// diagonal entry.
fn hessian_text(h: &Hessian) -> String {
    let nc = h.cc.n();
    let nm = h.cm.len();
    let n = nc + nm;
    let [cx, cy, cz] = h.cc.dims;
    let mut out = String::new();
    for col in 0..n {
        let mut entries: Vec<(usize, f64)> = Vec::new();
        if col < nc {
            let (kx, ky, kz) = (col % cx, (col / cx) % cy, col / (cx * cy));
            // the stencil is symmetric: column `col`'s entries are row `col`'s
            let row = &h.cc.data[col * 343..(col + 1) * 343];
            for dz in 0..7 {
                let lz = kz as isize + dz as isize - 3;
                if lz < 0 || lz as usize >= cz {
                    continue;
                }
                for dy in 0..7 {
                    let ly = ky as isize + dy as isize - 3;
                    if ly < 0 || ly as usize >= cy {
                        continue;
                    }
                    for dx in 0..7 {
                        let lx = kx as isize + dx as isize - 3;
                        if lx < 0 || lx as usize >= cx {
                            continue;
                        }
                        let v = row[(dz * 7 + dy) * 7 + dx];
                        if v != 0.0 {
                            let r = (lz as usize * cy + ly as usize) * cx + lx as usize;
                            entries.push((r, v));
                        }
                    }
                }
            }
            for (p, c) in h.cm.iter().enumerate() {
                if c[col] != 0.0 {
                    entries.push((nc + p, c[col]));
                }
            }
        } else {
            let p = col - nc;
            for (r, &v) in h.cm[p].iter().enumerate() {
                if v != 0.0 {
                    entries.push((r, v));
                }
            }
            for q in 0..nm {
                let v = h.mm[q * nm + p];
                if v != 0.0 {
                    entries.push((nc + q, v));
                }
            }
        }
        entries.sort_by_key(|e| e.0);
        if col == n - 1 && entries.last().map(|e| e.0) != Some(n - 1) {
            entries.push((n - 1, 0.0));
        }
        for (r, v) in entries {
            out.push_str(&format!("{}  {}  {}\n", r + 1, col + 1, g8(v)));
        }
    }
    out
}

/// `--profile`'s file: the levels' durations, then the run's.
pub fn profile_text(durations: &[f64], total: f64) -> String {
    let mut s = String::new();
    for (l, d) in durations.iter().enumerate() {
        s.push_str(&format!(
            "Entry: Begins topup level {} Duration: {}\n",
            l + 1,
            g(*d)
        ));
    }
    s.push_str(&format!("Entry: Begins topup run Duration: {}\n", g(total)));
    s
}

/// Silences an unused import when the estimation's types change.
#[allow(dead_code)]
fn _types(_: &topup::Estimate) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_as_fsl_prints_them() {
        assert_eq!(g8(2.5867307e-16), "2.5867307e-16");
        assert_eq!(g8(1.0), "1");
        assert_eq!(g10(-1.5e-7), "-1.5e-07");
        assert!(profile_text(&[0.5], 1.0).ends_with("Entry: Begins topup run Duration: 1\n"));
    }
}
