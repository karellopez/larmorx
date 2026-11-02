// SPDX-License-Identifier: Apache-2.0
//! FLIRT's standard output as a run writes it (`specs/flirt.md` §2.3, §2.5, §9.4): the verbose
//! levels' lines, `-debug`'s lines and the stream's number notation, collected in order.
//!
//! Every line FLIRT prints on standard output goes through one [`Trace`]: the schedule's echo,
//! the commands' output and the lines the costs, optimisations and the search print at their
//! verbose level. The stream prints numbers as `%g` with 6 significant digits until the first
//! matrix is printed, then as `%.6f` for good (§2.5). The evaluation counter numbers the cost
//! evaluations a level of 5 or more prints (`<count> : <cost> :: <parameters>`); it is reset
//! when a new image pair reports `Previous scale used <n> cost function evaluations` (observed).
//!
//! Where a line is printed and at which level is FSL 6.0.7.23's as black-box runs show it
//! (`docs/findings/fsl-flirt-original.md`, "The verbose levels and -debug"); the levels inside
//! the search are the run's level lowered by 2 (§2.3).

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use super::mat::{self, Mat4};
use larmorx_mri::hmc::volume::Volume;

/// A run's standard output and its state.
#[derive(Debug, Default)]
pub struct Trace {
    /// `-verbose` (`-v` is 1).
    pub verbose: i32,
    /// `-debug`.
    pub debug: bool,
    fixed: AtomicBool,
    count: AtomicU64,
    buf: Mutex<String>,
}

impl Trace {
    pub fn new(verbose: i32, debug: bool) -> Trace {
        Trace {
            verbose,
            debug,
            ..Trace::default()
        }
    }

    /// Whether the run's level is at least `level`.
    pub fn at(&self, level: i32) -> bool {
        self.verbose >= level
    }

    /// Whether numbers print in fixed notation now.
    pub fn fixed(&self) -> bool {
        self.fixed.load(Ordering::Relaxed)
    }

    /// The stream switched to fixed notation (a matrix or a row printed).
    pub fn set_fixed(&self) {
        self.fixed.store(true, Ordering::Relaxed);
    }

    /// A number as the stream prints it now.
    pub fn num(&self, v: f64) -> String {
        if self.fixed() {
            mat::f6(v)
        } else {
            mat::g(v, 6)
        }
    }

    /// Numbers, each followed by one blank.
    pub fn nums(&self, v: impl IntoIterator<Item = f64>) -> String {
        let mut s = String::new();
        for x in v {
            s.push_str(&self.num(x));
            s.push(' ');
        }
        s
    }

    /// A matrix as the stream prints it (fixed notation, which stays on).
    pub fn matrix(&self, m: &Mat4) -> String {
        self.set_fixed();
        mat::print_text(m)
    }

    /// Text as it is.
    pub fn write(&self, s: &str) {
        if let Ok(mut b) = self.buf.lock() {
            b.push_str(s);
        }
    }

    /// A line.
    pub fn line(&self, s: &str) {
        if let Ok(mut b) = self.buf.lock() {
            b.push_str(s);
            b.push('\n');
        }
    }

    /// Everything written so far (the buffer emptied).
    pub fn take(&self) -> String {
        self.buf
            .lock()
            .map(|mut b| std::mem::take(&mut *b))
            .unwrap_or_default()
    }

    /// One more cost evaluation; returns its number.
    pub fn tick(&self) -> u64 {
        self.count.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// The evaluations counted since the last reset, the counter reset.
    pub fn reset_count(&self) -> u64 {
        self.count.swap(0, Ordering::Relaxed)
    }

    /// The evaluation line of a level of 5 or more: `<count> : <cost> :: <parameters>`.
    pub fn evaluation(&self, cost: f32, params: &[f64]) {
        let n = self.count.load(Ordering::Relaxed);
        self.line(&format!(
            "{n} : {} :: {}",
            self.num(f64::from(cost)),
            self.nums(params.iter().copied())
        ));
    }

    /// An image's four information lines (`<name>:: Size`, `Dims`, `ROI Size`, `Minimum and
    /// maximum intensities`): its grid, voxel sizes (and `dim4`), volumes and value range.
    pub fn info(
        &self,
        name: &str,
        shape: [usize; 3],
        dims: [f64; 4],
        volumes: usize,
        values: &[f32],
    ) {
        let (lo, hi) = min_max(values);
        let size = format!("({},{},{},{})", shape[0], shape[1], shape[2], volumes);
        self.line(&format!("{name}:: Size = {size}"));
        self.line(&format!(
            "{name}:: Dims = ({},{},{},{})",
            self.num(dims[0]),
            self.num(dims[1]),
            self.num(dims[2]),
            self.num(dims[3])
        ));
        self.line(&format!("{name}:: ROI Size = {size}"));
        self.line(&format!(
            "{name}:: Minimum and maximum intensities are: {} and {}",
            self.num(f64::from(lo)),
            self.num(f64::from(hi))
        ));
    }

    /// [`Trace::info`] of a volume of the run (`dim4` 1, one volume).
    pub fn volume_info(&self, name: &str, v: &Volume) {
        let d = v.voxel_size.map(f64::from);
        self.info(name, v.shape, [d[0], d[1], d[2], 1.0], 1, &v.data);
    }
}

/// The smallest and largest value (NaN left out; 0 and 0 for none).
pub fn min_max(values: &[f32]) -> (f32, f32) {
    let (lo, hi) = values
        .iter()
        .filter(|v| !v.is_nan())
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(l, h), &x| {
            (l.min(x), h.max(x))
        });
    if lo > hi { (0.0, 0.0) } else { (lo, hi) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stream_switches_notation_once() {
        let t = Trace::new(2, false);
        assert_eq!(t.num(3.3f32.into()), "3.3");
        let _ = t.matrix(&mat::IDENTITY);
        assert_eq!(t.num(3.3f32.into()), "3.300000");
        t.tick();
        t.tick();
        assert_eq!(t.reset_count(), 2);
        t.tick();
        t.evaluation(0.5, &[1.0, -2.0]);
        assert_eq!(t.take(), "1 : 0.500000 :: 1.000000 -2.000000 \n");
    }
}
