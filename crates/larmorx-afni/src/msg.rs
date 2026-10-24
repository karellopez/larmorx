// SPDX-License-Identifier: Apache-2.0
//! AFNI's messages (`specs/3dTshift.md` §7.1, "Messages", observed).
//!
//! Labelled messages start with `++ ` (information), ` + ` (more information),
//! `*+ WARNING: `, `** ERROR: ` or `** FATAL ERROR: `, and end with a newline (added unless the
//! text ends with one). `AFNI_MESSAGE_PREFIX` and `::` start every labelled message. AFNI
//! writes the labels of warnings and errors in inverse video (`ESC[7m*+ WARNING:ESC[0m `), also
//! into files and pipes, unless `AFNI_MESSAGE_COLORIZE` is no: that is AFNI-21, a message-only
//! defect, so larmorx writes the colours in `reproduce` mode only. Some text AFNI prints as it
//! is, without a label or prefix ([`Message::Raw`]); standard output has no labels either.

use std::io::Write;

use larmorx_core::defects::{Notice, UpstreamDefects};

use crate::settings::AfniSettings;

/// A message on standard error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    /// `++ text`
    Info(String),
    /// ` + text`
    More(String),
    /// `*+ WARNING: text`
    Warning(String),
    /// `** ERROR: text`
    Error(String),
    /// Text as it is (it ends with a newline unless one is added).
    Raw(String),
    /// Text as it is, without a newline added (3dUnifize's progress letters).
    Text(String),
}

/// Writes 3dTshift's messages as AFNI does.
pub struct Log<'a> {
    out: &'a mut dyn Write,
    err: &'a mut dyn Write,
    prefix: Option<String>,
    colour: bool,
    /// `-verbose`.
    pub verbose: bool,
}

impl<'a> Log<'a> {
    /// Messages with `settings`' prefix, in colour when `mode` reproduces AFNI and
    /// `AFNI_MESSAGE_COLORIZE` is not no.
    pub fn new(
        out: &'a mut dyn Write,
        err: &'a mut dyn Write,
        settings: &AfniSettings,
        mode: UpstreamDefects,
    ) -> Log<'a> {
        Log {
            out,
            err,
            prefix: settings.get("AFNI_MESSAGE_PREFIX").map(str::to_owned),
            colour: mode.reproduce() && !settings.no("AFNI_MESSAGE_COLORIZE"),
            verbose: false,
        }
    }

    /// Applies the settings again (after the global options changed them).
    pub fn configure(&mut self, settings: &AfniSettings, mode: UpstreamDefects) {
        self.prefix = settings.get("AFNI_MESSAGE_PREFIX").map(str::to_owned);
        self.colour = mode.reproduce() && !settings.no("AFNI_MESSAGE_COLORIZE");
    }

    fn line(text: &str) -> String {
        if text.ends_with('\n') {
            text.to_owned()
        } else {
            format!("{text}\n")
        }
    }

    fn labelled(&mut self, label: &str, coloured: bool, text: &str) {
        let mut s = String::new();
        if let Some(p) = &self.prefix {
            s.push_str(p);
            s.push_str("::");
        }
        if coloured && self.colour {
            s.push_str("\x1b[7m");
            s.push_str(label.trim_end_matches(' '));
            s.push_str("\x1b[0m ");
        } else {
            s.push_str(label);
        }
        s.push_str(&Self::line(text));
        let _ = self.err.write_all(s.as_bytes());
    }

    /// `++ text` on standard error.
    pub fn info(&mut self, text: &str) {
        self.labelled("++ ", false, text);
    }

    /// ` + text` on standard error.
    pub fn more(&mut self, text: &str) {
        self.labelled(" + ", false, text);
    }

    pub fn warning(&mut self, text: &str) {
        self.labelled("*+ WARNING: ", true, text);
    }

    pub fn error(&mut self, text: &str) {
        self.labelled("** ERROR: ", true, text);
    }

    pub fn fatal(&mut self, text: &str) {
        self.labelled("** FATAL ERROR: ", true, text);
    }

    /// Text on standard error as it is.
    pub fn raw(&mut self, text: &str) {
        let _ = self.err.write_all(Self::line(text).as_bytes());
    }

    /// A line on standard output.
    pub fn stdout(&mut self, text: &str) {
        let _ = self.out.write_all(Self::line(text).as_bytes());
    }

    /// A line on standard output with `-verbose`.
    pub fn progress(&mut self, text: &str) {
        if self.verbose {
            self.stdout(text);
        }
    }

    /// An upstream-defect notice, as a warning (larmorx's line).
    pub fn notice(&mut self, notice: &Notice) {
        self.warning(&notice.line());
    }

    /// A message collected while reading.
    pub fn message(&mut self, m: &Message) {
        match m {
            Message::Info(t) => self.info(t),
            Message::More(t) => self.more(t),
            Message::Warning(t) => self.warning(t),
            Message::Error(t) => self.error(t),
            Message::Raw(t) => self.raw(t),
            Message::Text(t) => {
                let _ = self.err.write_all(t.as_bytes());
            }
        }
    }

    /// larmorx's own information line (`-verbose`): plain, on standard error.
    pub fn larmorx_info(&mut self, text: &str) {
        if self.verbose {
            let _ = self
                .err
                .write_all(Self::line(&format!("++ {text}")).as_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(settings: &AfniSettings, mode: UpstreamDefects, f: impl FnOnce(&mut Log<'_>)) -> String {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let mut log = Log::new(&mut out, &mut err, settings, mode);
        f(&mut log);
        String::from_utf8(err).unwrap()
    }

    #[test]
    fn labels_prefix_and_colour() {
        let none = AfniSettings::new();
        let text = run(&none, UpstreamDefects::Reproduce, |l| {
            l.warning("w");
            l.fatal("f");
            l.info("i");
            l.raw("r");
        });
        assert_eq!(
            text,
            "\x1b[7m*+ WARNING:\x1b[0m w\n\x1b[7m** FATAL ERROR:\x1b[0m f\n++ i\nr\n"
        );
        let plain = run(&none, UpstreamDefects::Fix, |l| l.error("e\n"));
        assert_eq!(plain, "** ERROR: e\n");
        let s = AfniSettings::from_pairs([
            ("AFNI_MESSAGE_PREFIX", "run1"),
            ("AFNI_MESSAGE_COLORIZE", "NO"),
        ]);
        let text = run(&s, UpstreamDefects::Reproduce, |l| {
            l.warning("w");
            l.more("m");
            l.raw("r");
        });
        assert_eq!(text, "run1::*+ WARNING: w\nrun1:: + m\nr\n");
    }
}
