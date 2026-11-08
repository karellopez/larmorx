// SPDX-License-Identifier: GPL-3.0-only
// Translated from tedana 26.0.3 (Copyright 2023, The ME-ICA Developers; GNU LGPL 2.1), under the
// GNU GPL version 3 as section 3 of the LGPL 2.1 allows; modified 2026 by Karel Lopez Vilaret.
//! What `t2smap` writes on standard error besides argparse's messages: tedana's `GENERAL` logger
//! (`tedana.utils.setup_loggers`: a stream handler with the format `%(levelname)-8s
//! %(module)s:%(funcName)s:%(lineno)d %(message)s`, level `INFO`, `WARNING` with `--quiet`,
//! `DEBUG` with `--debug`; its `REPORT` logger has no handler and prints nothing), Python's
//! warnings (`<file>:<line>: <Category>: <message>` and the source line, once per place and
//! message), the progress bars of `tqdm`, and an uncaught exception's last line.
//!
//! Where Python prints a path of its installation (warnings, tracebacks), the replica prints
//! the module's path inside tedana (`tedana/decay.py`) or its own name; timings in progress bars
//! are the replica's own (a documented difference). The bars are left out, as `tqdm` leaves
//! them out, when the environment disables them (`TQDM_DISABLE`, [`tqdm_disabled`]).

use std::collections::HashSet;
use std::io::Write;
use std::time::Instant;

/// A logging level (Python's numbers).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Debug = 10,
    Info = 20,
    Warning = 30,
}

impl Level {
    fn name(self) -> &'static str {
        match self {
            Level::Debug => "DEBUG",
            Level::Info => "INFO",
            Level::Warning => "WARNING",
        }
    }
}

/// Where tedana 26.0.3 logs a message: module, function and line, as `%(module)s:%(funcName)s:
/// %(lineno)d` prints them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Site(pub &'static str, pub &'static str, pub u32);

/// An exception tedana raises and nothing catches: Python prints the traceback and exits with
/// status 1. The replica prints the traceback's first and last lines (the exception's class and
/// message exactly) and, between them, where tedana raised it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PyError {
    /// The class as Python prints it (`ValueError`, `nibabel.filebasedimages.ImageFileError`).
    pub class: String,
    pub message: String,
    /// tedana's (or the library's) function that raised it, for the traceback's middle line.
    pub place: String,
}

impl PyError {
    pub fn new(class: &str, message: impl Into<String>, place: &str) -> PyError {
        PyError {
            class: class.to_owned(),
            message: message.into(),
            place: place.to_owned(),
        }
    }

    pub fn value_error(message: impl Into<String>, place: &str) -> PyError {
        PyError::new("ValueError", message, place)
    }

    /// The text Python prints (the call stack in between is not reproduced).
    pub fn traceback(&self) -> String {
        let last = if self.message.is_empty() {
            self.class.clone()
        } else {
            format!("{}: {}", self.class, self.message)
        };
        format!(
            "Traceback (most recent call last):\n  File \"{}\" (tedana 26.0.3's call stack is not \
             reproduced)\n{last}\n",
            self.place
        )
    }
}

/// tedana's logger and Python's warning registry for one run.
pub struct Log<'a> {
    pub err: &'a mut dyn Write,
    level: Option<Level>,
    warned: HashSet<(String, u32, String)>,
    /// The lines printed since [`Log::start_capture`] (progress bars left out: their timings
    /// are the replica's own), for the maths' comparison.
    capture: Option<Vec<String>>,
    /// The level and the warning registry when the capture started.
    at_capture: (Option<Level>, HashSet<(String, u32, String)>),
    /// Whether `tqdm`'s progress bars are printed (not when the environment disables them).
    bars: bool,
}

/// Whether `tqdm` 4.70.1 disables its bars for this environment: `tqdm.std.tqdm.__init__` takes
/// its defaults from the variables named `TQDM_<parameter>` (the rest of the name in any case,
/// `tqdm.utils.envwrap`, the last such variable winning), and converts the text for `disable`,
/// whose default is `False`, with `bool(text)`: any non-empty value disables the bars
/// (`TQDM_DISABLE=1`, also `TQDM_DISABLE=0`), an empty one does not.
pub fn tqdm_disabled(vars: impl IntoIterator<Item = (String, String)>) -> bool {
    let mut disabled = false;
    for (k, v) in vars {
        if k.len() > 5 && k.starts_with("TQDM_") && k[5..].to_lowercase() == "disable" {
            disabled = !v.is_empty();
        }
    }
    disabled
}

impl<'a> Log<'a> {
    /// Before `setup_loggers`: nothing is printed but warnings.
    pub fn new(err: &'a mut dyn Write) -> Log<'a> {
        // Python's os.environ on Windows holds the names in upper case.
        let bars = !tqdm_disabled(std::env::vars_os().filter_map(|(k, v)| {
            let k = k.into_string().ok()?;
            let k = if cfg!(windows) { k.to_uppercase() } else { k };
            Some((k, v.to_string_lossy().into_owned()))
        }));
        Log::with_bars(err, bars)
    }

    /// A logger that prints `tqdm`'s bars or not, whatever the environment says.
    pub fn with_bars(err: &'a mut dyn Write, bars: bool) -> Log<'a> {
        Log {
            err,
            level: None,
            warned: HashSet::new(),
            capture: None,
            at_capture: (None, HashSet::new()),
            bars,
        }
    }

    /// Keeps the lines printed from here on ([`Log::take_capture`]).
    pub fn start_capture(&mut self) {
        self.capture = Some(Vec::new());
        self.at_capture = (self.level, self.warned.clone());
    }

    /// The lines kept since [`Log::start_capture`]; stops keeping them.
    pub fn take_capture(&mut self) -> Vec<String> {
        self.capture.take().unwrap_or_default()
    }

    /// A logger on `err` in the state this one had when its capture started (level and
    /// warnings already printed), capturing: another pass of the same stages.
    pub fn fork<'b>(&self, err: &'b mut dyn Write) -> Log<'b> {
        Log {
            err,
            level: self.at_capture.0,
            warned: self.at_capture.1.clone(),
            capture: Some(Vec::new()),
            at_capture: self.at_capture.clone(),
            bars: self.bars,
        }
    }

    fn line(&mut self, text: String) {
        let _ = writeln!(self.err, "{text}");
        if let Some(c) = &mut self.capture {
            c.push(text);
        }
    }

    /// `setup_loggers(quiet=quiet, debug=debug)`.
    pub fn setup(&mut self, quiet: bool, debug: bool) {
        self.level = Some(if quiet {
            Level::Warning
        } else if debug {
            Level::Debug
        } else {
            Level::Info
        });
    }

    /// `teardown_loggers()`.
    pub fn teardown(&mut self) {
        self.level = None;
    }

    /// Whether a message of `level` is printed (`LGR.isEnabledFor`).
    pub fn enabled(&self, level: Level) -> bool {
        self.level.is_some_and(|l| level >= l)
    }

    fn emit(&mut self, level: Level, site: Site, msg: &str) {
        if self.enabled(level) {
            let Site(module, func, line) = site;
            self.line(format!("{:<8} {module}:{func}:{line} {msg}", level.name()));
        }
    }

    pub fn info(&mut self, site: Site, msg: &str) {
        self.emit(Level::Info, site, msg);
    }

    pub fn warning(&mut self, site: Site, msg: &str) {
        self.emit(Level::Warning, site, msg);
    }

    /// `LGR.debug(...)`; `msg` is only built when printed.
    pub fn debug(&mut self, site: Site, msg: impl FnOnce() -> String) {
        if self.enabled(Level::Debug) {
            let m = msg();
            self.emit(Level::Debug, site, &m);
        }
    }

    /// `warnings.warn(message, category)` attributed to `file:line` (whose source line is
    /// `source`), printed the first time this message comes from that place (Python's
    /// `default` action).
    pub fn warn(&mut self, file: &str, line: u32, category: &str, message: &str, source: &str) {
        if self
            .warned
            .insert((file.to_owned(), line, format!("{category}: {message}")))
        {
            self.line(format!("{file}:{line}: {category}: {message}\n  {source}"));
        }
    }

    /// The bar `tqdm(iterable, desc=desc)` prints for `total` items (`total` known), started.
    /// Nothing when the bars are disabled.
    pub fn progress(&mut self, desc: &str, total: usize) -> Progress {
        if self.bars {
            let _ = write!(
                self.err,
                "\r{desc}:   0%|          | 0/{total} [00:00<?, ?it/s]"
            );
        }
        Progress {
            desc: desc.to_owned(),
            total,
            start: Instant::now(),
            shown: self.bars,
        }
    }

    /// The bar's last state and its newline (`tqdm`'s close with `leave=True`).
    pub fn progress_done(&mut self, p: Progress) {
        if !p.shown {
            return;
        }
        let secs = p.start.elapsed().as_secs_f64();
        let elapsed = format!("{:02}:{:02}", (secs as u64) / 60, (secs as u64) % 60);
        let rate = if secs > 0.0 {
            p.total as f64 / secs
        } else {
            0.0
        };
        let rate = if rate >= 1.0 || rate == 0.0 {
            format!("{rate:.2}it/s")
        } else {
            format!("{:.2}s/it", 1.0 / rate)
        };
        let pct = if p.total == 0 { 0 } else { 100 };
        let bar = if p.total == 0 {
            "          "
        } else {
            "██████████"
        };
        let _ = writeln!(
            self.err,
            "\r{}: {pct:3}%|{bar}| {}/{} [{elapsed}<00:00, {rate}]",
            p.desc, p.total, p.total
        );
    }
}

/// A progress bar being shown.
pub struct Progress {
    desc: String,
    total: usize,
    start: Instant,
    shown: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn tqdm_disable_is_any_non_empty_value() {
        assert!(!tqdm_disabled(vars(&[])));
        assert!(tqdm_disabled(vars(&[("TQDM_DISABLE", "1")])));
        assert!(tqdm_disabled(vars(&[("TQDM_DISABLE", "0")])));
        assert!(tqdm_disabled(vars(&[("TQDM_disable", "yes")])));
        assert!(!tqdm_disabled(vars(&[("TQDM_DISABLE", "")])));
        assert!(!tqdm_disabled(vars(&[
            ("TQDM_DISABLE", "1"),
            ("TQDM_Disable", "")
        ])));
        assert!(!tqdm_disabled(vars(&[
            ("TQDM_DISABLED", "1"),
            ("tqdm_disable", "1")
        ])));
    }

    #[test]
    fn disabled_bars_print_nothing() {
        let mut shown = Vec::new();
        let mut log = Log::with_bars(&mut shown, true);
        let p = log.progress("2-echo monoexponential", 3);
        log.progress_done(p);
        drop(log);
        let text = String::from_utf8(shown).unwrap();
        assert!(text.contains("2-echo monoexponential: 100%|"), "{text}");
        let mut hidden = Vec::new();
        let mut log = Log::with_bars(&mut hidden, false);
        let p = log.progress("2-echo monoexponential", 3);
        log.progress_done(p);
        drop(log);
        assert!(hidden.is_empty());
    }
}
