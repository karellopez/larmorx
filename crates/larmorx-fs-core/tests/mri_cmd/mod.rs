// SPDX-License-Identifier: Apache-2.0
//! The black-box probe's commands over fs.mri's clean room (`specs/freesurfer-mri.md` §23; the
//! commands and encodings: `validation/investigations/freesurfer-mri-probe/README.md`): one
//! command line in, the probe's result line out (`<result> | x<standard output> x<standard
//! error>`), each line on a process state of its own (the probe's child process), the notices of
//! the defects a line met added to its standard error (`upstream defect ...`).
//!
//! Shared by the golden cases (`tests/golden_mri.rs`) and the comparison's candidate program
//! (`validation/investigations/freesurfer-cleanroom-mri/candidate`, which includes this folder by
//! path), so both run the same code.

pub mod args;
pub mod geo;
pub mod reg;
pub mod vol;

use std::io::Write;
use std::sync::{Arc, Mutex};

use larmorx_core::defects::UpstreamDefects;
use larmorx_fs_core::{Environment, Process, Stop};

pub use args::{Line, hexbytes};

/// A stream that keeps what is written.
#[derive(Clone, Default)]
struct Buf(Arc<Mutex<Vec<u8>>>);

impl Write for Buf {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("buffer").extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// One call: the command's printed values, or the end of the program.
fn call(p: &mut Process, l: &mut Line) -> Result<(), Stop> {
    let c = l.tok().to_owned();
    if vol::call(p, l, &c)? || geo::call(p, l, &c)? || reg::call(p, l, &c)? {
        return Ok(());
    }
    args::bad("unknown command");
}

/// One line of a batch (its tokens, the `@<seconds>` and `+gdiag=<hex>` prefixes included) on a
/// fresh process state in `mode`: the probe's result line.
pub fn run_line(toks: &[String], mode: UpstreamDefects, env: &Environment) -> String {
    let mut first = 0;
    let mut gdiag = 0u64;
    while first < toks.len() {
        let t = &toks[first];
        if t.starts_with('@') {
        } else if let Some(h) = t.strip_prefix("+gdiag=") {
            gdiag = u64::from_str_radix(h, 16).unwrap_or(0);
        } else {
            break;
        }
        first += 1;
    }
    let (o, e) = (Buf::default(), Buf::default());
    let mut p = Process::new(env.clone(), mode, Box::new(o.clone()), Box::new(e.clone()));
    p.gdiag = gdiag;
    let mut l = Line::new(toks[first..].to_vec());
    let r = call(&mut p, &mut l);
    p.flush();
    let res = match r {
        Ok(()) => l.result(),
        Err(s) if s.status() == 0 => l.result(),
        Err(s) => format!("EXIT {}", s.status()),
    };
    let notices = p.take_notices();
    let mut err = e.0.lock().expect("buffer").clone();
    for n in notices.as_slice() {
        err.extend_from_slice(n.line().as_bytes());
        err.push(b'\n');
    }
    let out = o.0.lock().expect("buffer").clone();
    format!("{res} | {} {}", hexbytes(&out), hexbytes(&err))
}
