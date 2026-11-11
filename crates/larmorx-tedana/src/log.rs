// SPDX-License-Identifier: Apache-2.0
//! The messages of `specs/t2smap.md` §12: tedana's log lines, Python's warnings and the errors.

use std::collections::HashSet;
use std::io::Write;

/// A log level (Python's `logging` numbers).
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

/// tedana's log on standard error (§12.1) and Python's warnings (§12.3), written as they happen.
pub struct Log<'a> {
    err: &'a mut (dyn Write + Send),
    threshold: Level,
    warned: HashSet<(&'static str, String)>,
    /// Every line written, for the Python API's result (the log).
    pub lines: Vec<String>,
}

impl<'a> Log<'a> {
    /// The log at INFO, DEBUG (`--debug`) or WARNING (`--quiet`, which wins).
    pub fn new(err: &'a mut (dyn Write + Send), debug: bool, quiet: bool) -> Log<'a> {
        let threshold = if quiet {
            Level::Warning
        } else if debug {
            Level::Debug
        } else {
            Level::Info
        };
        Log {
            err,
            threshold,
            warned: HashSet::new(),
            lines: Vec::new(),
        }
    }

    /// Whether DEBUG lines are written.
    pub fn debugging(&self) -> bool {
        self.threshold <= Level::Debug
    }

    /// One line `<LEVEL padded to 8> <module>:<function>:<line> <message>` (§12.1), `prefix`
    /// being `<module>:<function>:<line>` (§12.2).
    pub fn line(&mut self, level: Level, prefix: &str, message: &str) {
        if level < self.threshold {
            return;
        }
        let text = format!("{:<8} {prefix} {message}", level.name());
        self.raw(&text);
    }

    pub fn info(&mut self, prefix: &str, message: &str) {
        self.line(Level::Info, prefix, message);
    }

    pub fn warning(&mut self, prefix: &str, message: &str) {
        self.line(Level::Warning, prefix, message);
    }

    pub fn debug(&mut self, prefix: &str, message: &str) {
        self.line(Level::Debug, prefix, message);
    }

    /// A Python warning (`<place>: <Category>: <message>` and its source line), once per place
    /// and message as Python's default filter shows them (§12.3). The place is larmorx's own:
    /// the location lines are not part of the contract.
    pub fn py_warning(&mut self, place: &'static str, category: &str, message: &str) {
        if !self.warned.insert((place, message.to_owned())) {
            return;
        }
        let text = format!("larmorx/tedana/{place}: {category}: {message}\n  {place}");
        self.raw(&text);
    }

    /// Python's warnings registry reset (its filters changed): every place shows again.
    pub fn reset_registry(&mut self) {
        self.warned.clear();
    }

    /// Text written as it is (with a final newline).
    pub fn raw(&mut self, text: &str) {
        let _ = writeln!(self.err, "{text}");
        self.lines.extend(text.lines().map(str::to_owned));
    }
}

/// A Python exception: tedana's last traceback line `<class>: <message>` (§12.4), with the
/// exception it was raised while handling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PyError {
    pub class: String,
    pub message: String,
    pub context: Option<Box<PyError>>,
}

impl PyError {
    pub fn new(class: &str, message: impl Into<String>) -> PyError {
        PyError {
            class: class.to_owned(),
            message: message.into(),
            context: None,
        }
    }

    pub fn value(message: impl Into<String>) -> PyError {
        PyError::new("ValueError", message)
    }

    /// This error, raised while handling `context`.
    pub fn during(mut self, context: PyError) -> PyError {
        self.context = Some(Box::new(context));
        self
    }

    /// The last line of the traceback.
    pub fn last_line(&self) -> String {
        if self.message.is_empty() {
            self.class.clone()
        } else {
            format!("{}: {}", self.class, self.message)
        }
    }

    /// The traceback as the original prints it: the first line, a stand-in for Python's stack
    /// (an indented line: the stack is not part of the contract, §12.4) and the last line; a
    /// chained error after its context, joined by Python's sentence.
    pub fn traceback(&self) -> String {
        let mut s = String::new();
        if let Some(c) = &self.context {
            s.push_str(&c.traceback());
            s.push_str(
                "\n\nDuring handling of the above exception, another exception occurred:\n\n",
            );
        }
        s.push_str("Traceback (most recent call last):\n");
        s.push_str("  (larmorx's clean-room t2smap: no Python stack)\n");
        s.push_str(&self.last_line());
        s
    }
}

impl std::fmt::Display for PyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.last_line())
    }
}

impl std::error::Error for PyError {}

/// Python's `repr` of a list of strings (`['a', 'b']`, §12.2).
pub fn py_list_str(items: &[String]) -> String {
    let parts: Vec<String> = items
        .iter()
        .map(|s| larmorx_core::python::argparse::py_repr_str(s))
        .collect();
    format!("[{}]", parts.join(", "))
}

/// Python's `repr` of a list of integers.
pub fn py_list_int(items: &[i128]) -> String {
    let parts: Vec<String> = items.iter().map(i128::to_string).collect();
    format!("[{}]", parts.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_form_and_levels() {
        let mut buf = Vec::new();
        {
            let mut log = Log::new(&mut buf, false, false);
            log.info("t2smap:t2smap_workflow:365", "Using output directory: /x");
            log.debug("t2smap:t2smap_workflow:422", "hidden");
            log.warning("utils:check_te_values:796", "w");
            log.py_warning("adaptive.rs:1", "RuntimeWarning", "Mean of empty slice");
            log.py_warning("adaptive.rs:1", "RuntimeWarning", "Mean of empty slice");
        }
        let text = String::from_utf8(buf).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines[0],
            "INFO     t2smap:t2smap_workflow:365 Using output directory: /x"
        );
        assert_eq!(lines[1], "WARNING  utils:check_te_values:796 w");
        assert_eq!(lines.len(), 4);
        let mut buf = Vec::new();
        Log::new(&mut buf, true, true).info("a:b:1", "x");
        assert!(buf.is_empty(), "--quiet wins over --debug");
    }

    #[test]
    fn traceback_form() {
        let e = PyError::value("Expected 4D image").during(PyError::new("IndexError", "a"));
        let t = e.traceback();
        assert!(t.starts_with("Traceback (most recent call last):\n  "));
        assert!(t.ends_with("\nValueError: Expected 4D image"));
        assert!(t.contains("\nIndexError: a\n\nDuring handling of the above exception"));
        assert_eq!(py_list_str(&["a'b".into(), "c".into()]), "[\"a'b\", 'c']");
    }
}
