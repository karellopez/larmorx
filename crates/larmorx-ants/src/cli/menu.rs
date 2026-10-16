// SPDX-License-Identifier: Apache-2.0
//! ANTs' help menu (`CommandLineParser::PrintMenu`, `Utilities/antsCommandLineParser.cxx`,
//! ANTs v2.6.5), printed exactly as ANTs prints it: the command and its description, then each
//! option with its usages, its description broken into lines of at most 80 characters
//! (`BreakUpStringIntoNewLines`) and its values (`<VALUES>`: the values given on the command
//! line, newest first, then the option's default).

use std::io::Write;

use super::parser::{OptionSpec, OptionValue, Parsed};

/// One option as ANTs declares it (`OptionType`): names, usage lines, description, default
/// values (`AddFunction` at declaration).
#[derive(Clone, Copy, Debug)]
pub struct MenuOption {
    pub long: &'static str,
    pub short: Option<char>,
    pub usage: &'static [&'static str],
    pub description: &'static str,
    pub defaults: &'static [&'static str],
}

impl MenuOption {
    /// The parser's view of the option.
    pub const fn spec(&self) -> OptionSpec {
        OptionSpec {
            long: self.long,
            short: self.short,
        }
    }
}

/// `TokenizeString(str, tokens, " ")`: the words between blanks.
fn tokens(s: &str) -> Vec<&str> {
    s.split(' ').filter(|t| !t.is_empty()).collect()
}

/// `BreakUpStringIntoNewLines(longString, indentString, 80)`: words joined by a blank, a new
/// line (and the indent) before a word that would pass 80 characters on the current line; a
/// word of 80 characters or more on a line of its own.
pub fn break_up(long: &str, indent: &str, width: usize) -> String {
    let words = tokens(long);
    let mut out = String::new();
    let mut i = 0;
    let mut line = 0;
    while i < words.len() {
        let w = words[i];
        if w.len() >= width {
            out.push('\n');
            out.push_str(w);
            out.push('\n');
            i += 1;
            line = 0;
        } else if line + w.len() > width {
            out.push('\n');
            out.push_str(indent);
            line = 0;
        } else {
            out.push_str(w);
            out.push(' ');
            line += w.len() + 1;
            i += 1;
        }
    }
    out
}

/// A value as `PrintMenu` prints it: its name, and its parameters in brackets.
fn value_text(v: &OptionValue) -> String {
    if v.parameters.is_empty() {
        v.name.clone()
    } else {
        format!("{}[{}]", v.name, v.parameters.join(","))
    }
}

/// `PrintMenu(os, Indent(indent), printShortVersion)` for `command` with `description` and
/// `options` (in declaration order). `parsed` gives the values of the current command line.
pub fn print_menu(
    out: &mut dyn Write,
    command: &str,
    description: &str,
    options: &[MenuOption],
    parsed: Option<&Parsed>,
    indent: usize,
    short: bool,
) {
    let pad = |n: usize| " ".repeat(n);
    let mut s = String::from("\nCOMMAND: \n");
    s.push_str(&pad(indent));
    s.push_str(command);
    s.push('\n');
    if !description.is_empty() && !short {
        s.push_str(&pad(2 * indent));
        s.push_str(&break_up(description, &pad(2 * indent), 80));
        s.push('\n');
    }
    s.push_str("\nOPTIONS: \n");
    for o in options {
        s.push_str(&pad(indent));
        let mut continuation = indent;
        match o.short {
            Some(c) => {
                s.push('-');
                s.push(c);
                continuation += 2;
                if o.long.is_empty() {
                    s.push(' ');
                    continuation += 1;
                } else {
                    s.push_str(", --");
                    s.push_str(o.long);
                    s.push(' ');
                    continuation += 5 + o.long.len();
                }
            }
            None => {
                s.push_str("--");
                s.push_str(o.long);
                s.push(' ');
                continuation += 3 + o.long.len();
            }
        }
        match o.usage.split_first() {
            Some((first, rest)) => {
                s.push_str(first);
                s.push('\n');
                for u in rest {
                    s.push_str(&pad(continuation));
                    s.push_str(u);
                    s.push('\n');
                }
            }
            None => s.push('\n'),
        }
        if !o.description.is_empty() && !short {
            s.push_str(&pad(2 * indent));
            s.push_str(&break_up(o.description, &pad(2 * indent), 80));
            s.push('\n');
        }
        if !short {
            // The option's functions: the command line's, newest first, then the default.
            let mut values: Vec<String> = parsed
                .map(|p| p.all(o.long).iter().rev().map(value_text).collect())
                .unwrap_or_default();
            values.extend(o.defaults.iter().rev().map(|d| d.to_string()));
            match values.len() {
                0 => {}
                1 => {
                    s.push_str(&pad(2 * indent));
                    s.push_str("<VALUES>: ");
                    s.push_str(&values[0]);
                    s.push('\n');
                }
                _ => {
                    s.push_str(&pad(2 * indent));
                    s.push_str("<VALUES>: ");
                    s.push_str(&values.join(", "));
                }
            }
            s.push('\n');
        }
    }
    let _ = out.write_all(s.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_break_before_80_characters() {
        let text = "word ".repeat(30);
        let broken = break_up(&text, "          ", 80);
        let lines: Vec<&str> = broken.split('\n').collect();
        assert_eq!(lines[0].len(), 80);
        assert!(lines[1].starts_with("          word"));
        let long = "x".repeat(85);
        assert_eq!(
            break_up(&format!("a {long} b"), "  ", 80),
            format!("a \n{long}\nb ")
        );
    }
}
