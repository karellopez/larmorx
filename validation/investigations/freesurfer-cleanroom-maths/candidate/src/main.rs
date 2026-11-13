// SPDX-License-Identifier: Apache-2.0
//! The candidate of `specs/freesurfer-maths.md` §23: larmorx's clean room of FreeSurfer's
//! mathematics (`larmorx-fs-core`'s `maths`) behind the commands and print forms of the
//! black-box probe `fs_maths_probe` (`validation/investigations/freesurfer-maths-probe/README.md`),
//! so that `compare.py compare` runs both under the same commands.
//!
//! `fs-maths-candidate batch <file>`: one result line per line of the file, as the probe prints
//! it (`<result> | x<standard output> x<standard error>`); each line runs on a process state of
//! its own (the probe's child process), `LARMORX_UPSTREAM_DEFECTS` chooses the mode, and the
//! notices of the defects a line met are added to its standard error (`upstream defect ...`).

mod args;
mod hist;
mod mat;

use std::io::Write;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};

use larmorx_core::defects::UpstreamDefects;
use larmorx_fs_core::{Environment, Process, Stop};

pub use args::Line;

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

fn hexbytes(b: &[u8]) -> String {
    let mut o = String::with_capacity(1 + 2 * b.len());
    o.push('x');
    for x in b {
        o.push_str(&format!("{x:02x}"));
    }
    o
}

/// One call: the command's printed values, or the end of the program.
fn call(p: &mut Process, l: &mut Line) -> Result<(), Stop> {
    let c = l.tok().to_owned();
    if hist::call(p, l, &c)? || mat::call(p, l, &c)? {
        return Ok(());
    }
    eprintln!("candidate: unknown command {c}");
    Err(Stop::Exit(3))
}

fn run_line(toks: &[String], mode: UpstreamDefects, env: &Environment) -> String {
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

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 || args[1] != "batch" {
        eprintln!("usage: fs-maths-candidate batch <file>");
        return ExitCode::from(2);
    }
    let mode = UpstreamDefects::from_env().unwrap_or_default();
    let text = match std::fs::read(&args[2]) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("candidate: cannot open {}: {e}", args[2]);
            return ExitCode::from(2);
        }
    };
    let env = Environment::from_process();
    let mut out = std::io::BufWriter::new(std::io::stdout());
    let mut lines: Vec<&[u8]> = text.split(|&b| b == b'\n').collect();
    if text.ends_with(b"\n") {
        lines.pop();
    }
    for line in lines {
        let toks: Vec<String> = line
            .split(|&b| b == b' ' || b == b'\t')
            .filter(|t| !t.is_empty())
            .map(|t| String::from_utf8_lossy(t).into_owned())
            .collect();
        if toks.is_empty() {
            let _ = writeln!(out);
            continue;
        }
        let _ = writeln!(out, "{}", run_line(&toks, mode, &env));
    }
    let _ = out.flush();
    ExitCode::SUCCESS
}
