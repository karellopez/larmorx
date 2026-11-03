// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret.
//! A bit-exact replica of FSL's topup (FSL 6.0.7.23: fsl-topup 2203.6), the estimation of the
//! susceptibility-induced off-resonance field from images acquired with reversed
//! phase-encoding directions, translated from FSL's source under the FSL Licence's terms
//! (non-commercial; the licence and FSL's original source travel with it: `crates-nc/LICENSE`,
//! `crates-nc/fsl-source/`). FSL's libraries (newimage, NewNifti, utils, basisfield, warpfns,
//! miscmaths' sparse matrices and non-linear optimisers) are the shared core's
//! (`larmorx-nc-core`).
//!
//! larmorx (Apache-2.0) never links this crate: it runs the `larmorx-nc` program
//! (`larmorx-nc fsl topup`) as a separate process (`docs/licensing.md`).
//!
//! - [`topup`]: the program (`main`): the input, the resolution levels, the outputs.
//! - [`options`]: the command line and the configuration files.
//! - [`costfn`]: the cost function, its gradient and Hessian, and the files it writes.
//! - [`scan`]: the scans (resampling, derivatives, Jacobians, masks) and their mean.
//! - [`fileio`]: the acquisition-parameter file, the coefficient and field images.
//! - [`vol`]: newimage's `float` arithmetic.
//! - [`cli`]: the command line (`fsl topup`) as `larmorx-nc` runs it.
#![forbid(unsafe_code)]
// Index loops mirror FSL's loops, so that each step can be checked against the source.
#![allow(clippy::needless_range_loop)]

use std::cell::RefCell;
use std::io::Write;

pub mod cli;
pub mod costfn;
pub mod fileio;
pub mod options;
pub mod scan;
pub mod topup;
pub mod vol;

pub use cli::topup;

/// The program's two streams (`cout`, `cerr`), shared by every part that prints.
pub struct Console<'a> {
    out: RefCell<&'a mut dyn Write>,
    err: RefCell<&'a mut dyn Write>,
}

impl<'a> Console<'a> {
    pub fn new(out: &'a mut dyn Write, err: &'a mut dyn Write) -> Console<'a> {
        Console {
            out: RefCell::new(out),
            err: RefCell::new(err),
        }
    }

    /// `cout << s`.
    pub fn put(&self, s: &str) {
        let _ = self.out.borrow_mut().write_all(s.as_bytes());
    }

    /// `cout << s << endl`.
    pub fn line(&self, s: &str) {
        let mut o = self.out.borrow_mut();
        let _ = o.write_all(s.as_bytes());
        let _ = o.write_all(b"\n");
    }

    /// `if (TracePrint()) cout << s << endl`.
    pub fn trace(&self, on: bool, s: &str) {
        if on {
            self.line(s);
        }
    }

    /// `cerr << s`.
    pub fn err_put(&self, s: &str) {
        let _ = self.err.borrow_mut().write_all(s.as_bytes());
    }

    /// `cerr << s << endl`.
    pub fn err_line(&self, s: &str) {
        let mut e = self.err.borrow_mut();
        let _ = e.write_all(s.as_bytes());
        let _ = e.write_all(b"\n");
    }
}

/// A call of `exit()` deep inside the program (newimage's `FslGetEnvOutputType` with an
/// output type it cannot write): the lines printed and the status. It unwinds to `main`.
#[derive(Debug)]
pub struct ExitNow {
    pub code: u8,
    pub printed: Vec<String>,
}
