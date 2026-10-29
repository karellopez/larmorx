// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret (the translation).
//! FSL's command-line options (fsl-utils 2203.5: `options.h`, `parse.cc`, `usage.cc`,
//! `functions.cc`, `matches.cc`), as `bet2` and `betsurf` parse them.
//!
//! What the parser does (and so what the replica does):
//! - options are scanned from a given position; a word that does not start with `-` stops the
//!   scan with `<word>:  is an unrecognised token!`;
//! - a word starting with `--` is a long option, its value only after `=` (`--opt value` reads
//!   no value: `Missing non-optional argument`);
//! - any other word is a run of one-letter options (`-mn` is `-m -n`), each taking its values
//!   from the following words;
//! - numbers are read with `strtod` (floats, then rounded to `float`) or `strtol` with base 0
//!   (integers: `010` is 8), and must use the whole word; an option given twice keeps the last
//!   value (overwrite mode `Allow`);
//! - a value that does not read as a number is an error naming it and the word after it
//!   (`Couldn't set_value! valstr="<value> <next word>"`), except for an option that was set
//!   before, or a multi-value option whose last value reads (FSL keeps the numeric prefix);
//! - `usage()` prints the title, the example and every visible option to standard error.

use crate::cnum;

/// How many values an option takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgFlag {
    NoArgument,
    Requires(usize),
    Optional,
}

/// The type of an option's value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Bool,
    Float,
    Int,
    Str,
}

/// An option's value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Float(f32),
    Int(i32),
    Str(String),
}

/// One option: its keys (`-v,--verbose`), default, help text, and argument requirements.
#[derive(Clone, Debug)]
pub struct Opt {
    pub key: &'static str,
    pub default: Value,
    pub help: &'static str,
    pub compulsory: bool,
    pub flag: ArgFlag,
    pub visible: bool,
}

impl Opt {
    pub fn kind(&self) -> Kind {
        match self.default {
            Value::Bool(_) => Kind::Bool,
            Value::Float(_) => Kind::Float,
            Value::Int(_) => Kind::Int,
            Value::Str(_) => Kind::Str,
        }
    }

    fn matches(&self, arg: &str) -> bool {
        self.key.split(',').any(|k| k == arg)
    }

    fn nrequired(&self) -> usize {
        match self.flag {
            ArgFlag::NoArgument => 0,
            ArgFlag::Optional => 1,
            ArgFlag::Requires(n) => n,
        }
    }

    fn has_arg(&self) -> bool {
        self.flag != ArgFlag::NoArgument
    }

    /// `BaseOption::usage`.
    fn usage_line(&self) -> String {
        if let Some(rest) = self.help.strip_prefix('~') {
            format!("\t{} {}", self.key, rest)
        } else {
            format!("\t{}\t{}", self.key, self.help)
        }
    }
}

/// An option's state after parsing.
#[derive(Clone, Debug)]
pub struct OptState {
    pub set: bool,
    pub value: Value,
    /// The values of a multi-value option (`valuevec_`).
    pub values: Vec<Value>,
}

impl OptState {
    pub fn bool(&self) -> bool {
        matches!(self.value, Value::Bool(true))
    }

    pub fn float(&self) -> f32 {
        match self.value {
            Value::Float(v) => v,
            _ => 0.0,
        }
    }

    pub fn int(&self) -> i32 {
        match self.value {
            Value::Int(v) => v,
            _ => 0,
        }
    }

    pub fn str(&self) -> &str {
        match &self.value {
            Value::Str(s) => s,
            _ => "",
        }
    }

    /// `value(n)` of a multi-value option.
    pub fn float_at(&self, n: usize) -> f32 {
        match self.values.get(n) {
            Some(Value::Float(v)) => *v,
            _ => 0.0,
        }
    }
}

/// `string_to_T` for one value: the value it stores, and whether the whole word read.
fn string_to_t(kind: Kind, s: &str) -> (Value, bool) {
    match kind {
        Kind::Bool => (Value::Bool(false), false),
        Kind::Str => (Value::Str(s.to_owned()), true),
        Kind::Float => {
            let (v, end) = cnum::strtod(s);
            (Value::Float(v as f32), end == s.len())
        }
        Kind::Int => {
            let (v, end) = cnum::strtol(s, 0);
            (Value::Int(v as i32), end == s.len())
        }
    }
}

/// `OptionParser`.
pub struct OptionParser {
    pub title: String,
    pub examples: String,
    pub opts: Vec<Opt>,
}

/// The parse of a command line: each option's state (in `opts` order), and where the scan
/// stopped.
pub struct Parsed {
    pub states: Vec<OptState>,
    pub stop: usize,
}

impl Parsed {
    pub fn get(&self, parser: &OptionParser, key: &str) -> &OptState {
        let i = parser
            .opts
            .iter()
            .position(|o| o.key == key)
            .unwrap_or_else(|| panic!("no option {key}"));
        &self.states[i]
    }
}

impl OptionParser {
    pub fn new(title: impl Into<String>, examples: impl Into<String>, opts: Vec<Opt>) -> Self {
        OptionParser {
            title: title.into(),
            examples: examples.into(),
            opts,
        }
    }

    fn initial(&self) -> Vec<OptState> {
        self.opts
            .iter()
            .map(|o| OptState {
                set: false,
                value: o.default.clone(),
                values: Vec::new(),
            })
            .collect()
    }

    /// `parse_option`: the number of words used, or the exception's message.
    fn parse_option(
        &self,
        states: &mut [OptState],
        optstr: &str,
        valstr: &str,
        argv: &[String],
        valpos: usize,
    ) -> Result<usize, String> {
        let Some(i) = self.opts.iter().position(|o| o.matches(optstr)) else {
            return Err(format!("{optstr}: Option doesn't exist!"));
        };
        let o = &self.opts[i];
        if o.has_arg() {
            if !valstr.is_empty() {
                if self.set_value(i, &mut states[i], valstr, argv, valpos) {
                    return Ok(1 + o.nrequired());
                }
                let mut errstr = format!("Couldn't set_value! valstr=\"{valstr}");
                for nn in valpos + 1..=valpos + o.nrequired() {
                    if nn < argv.len() {
                        errstr.push(' ');
                        errstr.push_str(&argv[nn]);
                    }
                }
                return Err(format!("{optstr}: {errstr}\"!"));
            } else if o.flag != ArgFlag::Optional {
                return Err(format!("{optstr}: Missing non-optional argument!"));
            }
        }
        if o.flag == ArgFlag::Optional {
            states[i].set = true;
        } else {
            self.set_value_plain(i, &mut states[i], "");
        }
        Ok(1)
    }

    /// `Option<T>::set_value(const string&)`.
    fn set_value_plain(&self, i: usize, st: &mut OptState, s: &str) -> bool {
        let o = &self.opts[i];
        if o.kind() == Kind::Bool {
            let default = matches!(o.default, Value::Bool(true));
            if s.is_empty() {
                st.value = Value::Bool(!default);
                st.set = true;
            } else if s == "true" {
                st.value = Value::Bool(true);
                st.set = true;
            } else if s == "false" {
                st.value = Value::Bool(false);
                st.set = true;
            }
            return st.set;
        }
        let (v, ok) = string_to_t(o.kind(), s);
        st.value = v;
        if ok {
            st.set = true;
        }
        st.set
    }

    /// `Option<T>::set_value(vs, argv, valpos, argc)`.
    fn set_value(
        &self,
        i: usize,
        st: &mut OptState,
        vs: &str,
        argv: &[String],
        valpos: usize,
    ) -> bool {
        let o = &self.opts[i];
        if o.kind() == Kind::Bool {
            return self.set_value_plain(i, st, vs);
        }
        let n = o.nrequired();
        if n == 0 {
            return false;
        }
        if n == 1 {
            let (v, ok) = string_to_t(o.kind(), vs);
            st.value = v;
            if ok {
                st.set = true;
            }
            return st.set;
        }
        st.values.clear();
        for nv in 0..n {
            let word = argv.get(valpos + nv).map_or("", String::as_str);
            let (v, ok) = string_to_t(o.kind(), word);
            st.set = ok;
            st.values.push(v);
        }
        st.set
    }

    /// `parse_long_option`.
    fn parse_long_option(&self, states: &mut [OptState], s: &str) -> Result<usize, String> {
        let (optstr, valstr) = match s.find('=') {
            Some(p) => (&s[..p], &s[p + 1..]),
            None => (s, ""),
        };
        self.parse_option(states, optstr, valstr, &[], 0)?;
        Ok(1)
    }

    /// `parse_command_line(argc, argv, skip)` (`argv[0]` is the program).
    pub fn parse(&self, argv: &[String], skip: usize) -> Result<Parsed, (Vec<OptState>, String)> {
        self.parse_command_line(argv, skip, false)
    }

    /// `parse_command_line(argc, argv, skip, silentFail)`: with `silent_fail` a word that does
    /// not start with `-` ends the options (the arguments that follow are the caller's).
    pub fn parse_command_line(
        &self,
        argv: &[String],
        skip: usize,
        silent_fail: bool,
    ) -> Result<Parsed, (Vec<OptState>, String)> {
        let mut states = self.initial();
        let mut optpos = 1 + skip;
        while optpos < argv.len() {
            let optstr = argv[optpos].as_str();
            if !optstr.starts_with('-') {
                if silent_fail {
                    break;
                }
                return Err((states, format!("{optstr}:  is an unrecognised token!")));
            }
            if optstr.as_bytes().get(1) == Some(&b'-') {
                match self.parse_long_option(&mut states, optstr) {
                    Ok(n) => optpos += n,
                    Err(e) => return Err((states, e)),
                }
            } else {
                let mut valpos = optpos + 1;
                for ch in optstr[1..].chars() {
                    let sub = format!("-{ch}");
                    let valstr = argv.get(valpos).map_or("", String::as_str).to_owned();
                    match self.parse_option(&mut states, &sub, &valstr, argv, valpos) {
                        Ok(inc) => valpos += inc - 1,
                        Err(e) => return Err((states, e)),
                    }
                }
                optpos = valpos;
            }
        }
        Ok(Parsed {
            states,
            stop: optpos,
        })
    }

    /// `usage()` (to standard error).
    pub fn usage(&self) -> String {
        let mut s = String::new();
        // utils 2412.6's build string (`git describe` of its build); 2203.5 printed `""`
        s.push_str("\nPart of FSL (ID: 2412.6-dirty)\n");
        s.push_str(&self.title);
        s.push_str("\n\nUsage: \n");
        s.push_str(&self.examples);
        s.push('\n');
        s.push_str(&self.describe_options());
        s
    }

    /// `describe_options()`.
    fn describe_options(&self) -> String {
        let mut s = String::new();
        let mut banner = true;
        for o in self.opts.iter().filter(|o| o.compulsory && o.visible) {
            if banner {
                s.push_str("\nCompulsory arguments (You MUST set one or more of):\n");
                banner = false;
            }
            s.push_str(&o.usage_line());
            s.push('\n');
        }
        let mut banner = true;
        for o in self.opts.iter().filter(|o| !o.compulsory && o.visible) {
            if banner {
                s.push_str("\nOptional arguments (You may optionally specify one or more of):\n");
                banner = false;
            }
            s.push_str(&o.usage_line());
            s.push('\n');
        }
        s.push_str("\n\n");
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parser() -> OptionParser {
        OptionParser::new(
            "T",
            "ex",
            vec![
                Opt {
                    key: "-m,--mask",
                    default: Value::Bool(false),
                    help: "mask",
                    compulsory: false,
                    flag: ArgFlag::NoArgument,
                    visible: true,
                },
                Opt {
                    key: "-f",
                    default: Value::Float(0.5),
                    help: "~<f>\t\tfrac",
                    compulsory: false,
                    flag: ArgFlag::Requires(1),
                    visible: true,
                },
                Opt {
                    key: "-c",
                    default: Value::Float(0.0),
                    help: "~<x y z>\tcentre",
                    compulsory: false,
                    flag: ArgFlag::Requires(3),
                    visible: true,
                },
            ],
        )
    }

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn short_runs_and_values() {
        let p = parser();
        let r = p
            .parse(&args(&["bet2", "in", "out", "-mf", "0.2"]), 2)
            .ok()
            .unwrap();
        assert!(r.get(&p, "-m,--mask").bool());
        assert_eq!(r.get(&p, "-f").float(), 0.2f32);
        let r = p
            .parse(&args(&["bet2", "in", "out", "-c", "1", "2"]), 2)
            .ok()
            .unwrap();
        let c = r.get(&p, "-c");
        assert!(c.set);
        assert_eq!(
            (c.float_at(0), c.float_at(1), c.float_at(2)),
            (1.0, 2.0, 0.0)
        );
    }

    #[test]
    fn errors() {
        let p = parser();
        let e = |a: &[&str]| p.parse(&args(a), 2).err().unwrap().1;
        assert_eq!(
            e(&["bet2", "i", "o", "junk"]),
            "junk:  is an unrecognised token!"
        );
        assert_eq!(e(&["bet2", "i", "o", "-q"]), "-q: Option doesn't exist!");
        assert_eq!(
            e(&["bet2", "i", "o", "-f"]),
            "-f: Missing non-optional argument!"
        );
        assert_eq!(
            e(&["bet2", "i", "o", "-f", "x", "-m"]),
            "-f: Couldn't set_value! valstr=\"x -m\"!"
        );
        assert_eq!(
            e(&["bet2", "i", "o", "--mask=1", "-f"]),
            "-f: Missing non-optional argument!"
        );
    }

    #[test]
    fn usage_text() {
        let u = parser().usage();
        assert!(u.starts_with("\nPart of FSL (ID: 2412.6-dirty)\nT\n\nUsage: \nex\n"));
        assert!(u.contains("\t-f <f>\t\tfrac\n"));
        assert!(u.contains("\t-m,--mask\tmask\n"));
        assert!(u.ends_with("\n\n\n"));
    }
}
