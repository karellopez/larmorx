// SPDX-License-Identifier: Apache-2.0
// Translated to Rust from AFNI 26.3.05 src/debugtrace.c (output_message, INFO_message,
// ININFO_message, WARNING_message, ERROR_message, ERROR_exit). debugtrace.c carries no Medical
// College of Wisconsin notice: it was written at the NIH and is in the public domain (AFNI's
// LICENSE.txt).
// AFNI asks that work based on it cite its authors (LICENSE.txt).
// Rust translation Copyright 2026 Karel Lopez Vilaret, licensed under the Apache License 2.0;
// modified from the original: translated to Rust, with the changes PROVENANCE.md lists.
//! AFNI's messages: what each of its message routines prints, with `AFNI_MESSAGE_PREFIX` and
//! `AFNI_MESSAGE_COLORIZE`.
//!
//! Routines that read a file (`dataset::read`, `oned`) return the messages AFNI prints there as
//! [`Message`]s, and the command line prints them in order with [`Style::format`], as AFNI's
//! routine of each kind would:
//!
//! - `INFO_message` (`++ `), `ININFO_message` (` + `), `WARNING_message` (`*+ WARNING: `),
//!   `ERROR_message` and `ERROR_exit` (`** ERROR: `, `** FATAL ERROR: `) prefix the text with
//!   their label and with `AFNI_MESSAGE_PREFIX` followed by `::` when that variable is set, and
//!   end it with a newline unless it ends with one;
//! - AFNI shows the labels of warnings and errors in inverse video, with the terminal escape
//!   codes `ESC[7m` and `ESC[0m`, also when standard error is a file or a pipe, unless
//!   `AFNI_MESSAGE_COLORIZE` starts with `N` or `n`. larmorx does so in `reproduce` mode
//!   only: by default its messages are plain text (AFNI-21, a message-only defect);
//! - text AFNI prints with `fprintf(stderr, ...)` ([`Message::Raw`]) has neither.
//!
//! `ERROR_exit` also prints `** Program compile date = <the date AFNI was compiled>`, and
//! AFNI programs start with a version banner (`++ 3dTshift: AFNI version=...`). Both name the
//! build of AFNI, not what the program does; larmorx prints neither.

/// A message, by the AFNI routine that prints it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    /// `INFO_message`: `++ text`.
    Info(String),
    /// `ININFO_message`: ` + text`.
    InInfo(String),
    /// `WARNING_message`: `*+ WARNING: text`.
    Warning(String),
    /// `ERROR_message`: `** ERROR: text` (the program may go on).
    Error(String),
    /// `ERROR_exit`: `** FATAL ERROR: text`, then the program exits with status 1.
    Fatal(String),
    /// `fprintf(stderr, text)`: the text as it is (newlines included).
    Raw(String),
}

/// How messages are printed: `AFNI_MESSAGE_PREFIX` and whether labels are coloured.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Style {
    /// `AFNI_MESSAGE_PREFIX`, when set: written before every labelled message, with `::`.
    pub prefix: Option<String>,
    /// Whether warning and error labels get AFNI's inverse-video escape codes.
    pub colorize: bool,
}

impl Style {
    /// The text `message` puts on standard error.
    pub fn format(&self, message: &Message) -> String {
        let (label, colored, text) = match message {
            Message::Info(t) => ("++ ", false, t),
            Message::InInfo(t) => (" + ", false, t),
            Message::Warning(t) => ("*+ WARNING: ", true, t),
            Message::Error(t) => ("** ERROR: ", true, t),
            Message::Fatal(t) => ("** FATAL ERROR: ", true, t),
            Message::Raw(t) => return t.clone(),
        };
        let mut s = String::new();
        if let Some(p) = &self.prefix {
            s.push_str(p);
            s.push_str("::");
        }
        if colored && self.colorize {
            // "\033[7m" + the label without its last blank + "\033[0m ".
            s.push_str("\x1b[7m");
            s.push_str(label.strip_suffix(' ').unwrap_or(label));
            s.push_str("\x1b[0m ");
        } else {
            s.push_str(label);
        }
        s.push_str(text);
        if !s.ends_with('\n') {
            s.push('\n');
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_prefixes_and_colours() {
        let plain = Style::default();
        assert_eq!(
            plain.format(&Message::Warning("w".into())),
            "*+ WARNING: w\n"
        );
        assert_eq!(plain.format(&Message::Info("i\n".into())), "++ i\n");
        assert_eq!(plain.format(&Message::InInfo("r".into())), " + r\n");
        assert_eq!(plain.format(&Message::Raw("raw".into())), "raw");
        let afni = Style {
            prefix: Some("P Q".into()),
            colorize: true,
        };
        assert_eq!(
            afni.format(&Message::Fatal("Unknown option: -x".into())),
            "P Q::\x1b[7m** FATAL ERROR:\x1b[0m Unknown option: -x\n"
        );
        // Info messages are never coloured.
        assert_eq!(afni.format(&Message::Info("x".into())), "P Q::++ x\n");
        assert_eq!(afni.format(&Message::Raw("x\n".into())), "x\n");
    }
}
