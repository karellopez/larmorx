// SPDX-License-Identifier: Apache-2.0
//! A command line's output passed to Python's `sys.stdout` and `sys.stderr` while it runs (the
//! console script, `larmorx/cli/__init__.py`), so that a long tool's messages appear as they are
//! written, as they do from the standalone program, instead of all at its end.
//!
//! Standard output is line-buffered and standard error is not, as Rust's own streams are for the
//! standalone program; each piece written goes to `sys.<stream>.write` and is flushed. The text
//! is the bytes decoded as UTF-8 with invalid bytes replaced, as the collected output was: an
//! incomplete character at the end of a piece waits for the rest. The GIL is taken for each
//! piece only; the command runs without it.

use std::io::{self, Write};

use pyo3::prelude::*;

/// One of Python's output streams, as a [`Write`] for the command line.
pub struct PyStream {
    /// `"stdout"` or `"stderr"`: the attribute of `sys`.
    name: &'static str,
    /// Written up to the last end of line (standard output), or at once (standard error).
    line_buffered: bool,
    pending: Vec<u8>,
    /// The first Python exception a write raised (a closed pipe, `KeyboardInterrupt`): nothing
    /// more is written, and it is raised once the command has ended.
    failed: Option<PyErr>,
}

impl PyStream {
    /// `sys.stdout`, line-buffered.
    pub fn stdout() -> PyStream {
        PyStream::new("stdout", true)
    }

    /// `sys.stderr`, unbuffered.
    pub fn stderr() -> PyStream {
        PyStream::new("stderr", false)
    }

    fn new(name: &'static str, line_buffered: bool) -> PyStream {
        PyStream {
            name,
            line_buffered,
            pending: Vec::new(),
            failed: None,
        }
    }

    /// Writes what is left (an incomplete character too, replaced) and returns the first Python
    /// exception a write raised.
    pub fn finish(mut self) -> Option<PyErr> {
        let n = self.pending.len();
        self.emit(n, true);
        self.failed
    }

    /// Writes `pending[..upto]` to Python's stream, keeping an incomplete UTF-8 sequence at its
    /// end for the next piece unless `all`.
    fn emit(&mut self, upto: usize, all: bool) {
        let cut = if all {
            upto
        } else {
            upto - incomplete_tail(&self.pending[..upto])
        };
        if cut == 0 {
            return;
        }
        let text = String::from_utf8_lossy(&self.pending[..cut]).into_owned();
        self.pending.drain(..cut);
        if self.failed.is_some() {
            return;
        }
        let name = self.name;
        let written = Python::attach(|py| -> PyResult<()> {
            let stream = py.import("sys")?.getattr(name)?;
            stream.call_method1("write", (text,))?;
            stream.call_method0("flush")?;
            Ok(())
        });
        if let Err(e) = written {
            self.failed = Some(e);
        }
    }
}

/// The length of an incomplete UTF-8 sequence at the end of `bytes` (0 if there is none): a lead
/// byte followed by fewer continuation bytes than it announces.
fn incomplete_tail(bytes: &[u8]) -> usize {
    let start = bytes.len().saturating_sub(3);
    for i in (start..bytes.len()).rev() {
        let b = bytes[i];
        if b & 0xC0 == 0x80 {
            continue; // a continuation byte
        }
        let need = match b {
            0xC0..=0xDF => 2,
            0xE0..=0xEF => 3,
            0xF0..=0xF7 => 4,
            _ => return 0,
        };
        let have = bytes.len() - i;
        return if have < need { have } else { 0 };
    }
    0
}

impl Write for PyStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.pending.extend_from_slice(buf);
        let upto = if self.line_buffered {
            self.pending
                .iter()
                .rposition(|&b| b == b'\n')
                .map_or(0, |i| i + 1)
        } else {
            self.pending.len()
        };
        self.emit(upto, false);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let n = self.pending.len();
        self.emit(n, false);
        Ok(())
    }
}
