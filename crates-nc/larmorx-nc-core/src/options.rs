// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret (the translation).
//! FSL's command-line options (fsl-utils 2203.5: `options.h`, `parse.cc`, `usage.cc`,
//! `functions.cc`, `matches.cc`; `check.cc`'s compulsory options), as `bet2`, `betsurf` and
//! `makerot` parse them.
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
//! - a `std::vector<float>` option's value is one word split at `,` (at `:` when the word holds
//!   one), each piece read with `atof` (no check: `x` is 0), the vector cleared first: its old
//!   buffer is kept, so reading past the new values reads what an earlier value (or the default)
//!   left there ([`OptState::floats_buffer`]);
//! - a `std::vector<int>` option's value is split alike, each piece read with `atoi`;
//! - `usage()` prints the title, the example and every visible option to standard error;
//! - `parse_config_file` (topup, FNIRT) reads a file word by word: a word starting with `#`
//!   discards the rest of its line, one starting with `--` is a long option, any other word is
//!   an option whose value is the rest of its line; an option already set is left as it is
//!   (overwrite mode `Ignore`); a line of 1023 characters or more ends the reading (`getline`'s
//!   failure);
//! - `operator<<(ostream, option)` ([`Opt::print`]) writes `# <help>` and `--key=value` (the log
//!   files topup writes).

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
    FloatVec,
    IntVec,
}

/// An option's value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Float(f32),
    Int(i32),
    Str(String),
    /// `std::vector<float>`: its values.
    FloatVec(Vec<f32>),
    /// `std::vector<int>`: its values.
    IntVec(Vec<i32>),
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
            Value::FloatVec(_) => Kind::FloatVec,
            Value::IntVec(_) => Kind::IntVec,
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

    /// The first key of the short (`-x`) or long (`--x`) form (`short_form`, `long_form`).
    fn form(&self, long: bool) -> &str {
        self.key
            .split(',')
            .find(|k| k.starts_with("--") == long)
            .unwrap_or("")
    }

    /// `operator<<(ostream, option)` (`Option<T>::print`): `# <help>`, a newline, then
    /// `config_key()` and `value_string()`: `--key=value` (`-k value` without a long form); a
    /// `bool` option its key when set, nothing otherwise. Values as `ostream` writes them
    /// (floats `%g` with 6 digits, vectors joined by `,`).
    pub fn print(&self, st: &OptState) -> String {
        let mut s = format!("# {}\n", self.help);
        if self.kind() == Kind::Bool {
            if st.set {
                let k = self.form(true);
                s.push_str(if k.is_empty() { self.form(false) } else { k });
            }
            return s;
        }
        let long = self.form(true);
        if long.is_empty() {
            s.push_str(self.form(false));
            s.push(' ');
        } else {
            s.push_str(long);
            s.push('=');
        }
        let g = |v: f32| cnum::fmt_g(f64::from(v), 6);
        match &st.value {
            Value::Bool(_) => {}
            Value::Float(v) => s.push_str(&g(*v)),
            Value::Int(v) => s.push_str(&v.to_string()),
            Value::Str(v) => s.push_str(v),
            Value::FloatVec(v) => {
                s.push_str(&v.iter().map(|&x| g(x)).collect::<Vec<_>>().join(","));
            }
            Value::IntVec(v) => {
                s.push_str(&v.iter().map(i32::to_string).collect::<Vec<_>>().join(","));
            }
        }
        s
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
    /// A `std::vector<float>` option's buffer: every position any value (or the default) has
    /// written, in order; the vector's own values are its first [`OptState::floats`].
    pub buffer: Vec<f32>,
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

    /// A `std::vector<float>` option's values.
    pub fn floats(&self) -> &[f32] {
        match &self.value {
            Value::FloatVec(v) => v,
            _ => &[],
        }
    }

    /// `value()[k]` of a `std::vector<float>` option for `k < n`, read as libstdc++ reads it:
    /// past the values, what the vector's buffer still holds (a value parsed earlier, or the
    /// default's). `None`: beyond anything written (memory the vector never wrote).
    pub fn floats_buffer(&self, n: usize) -> Option<Vec<f32>> {
        (self.buffer.len() >= n).then(|| self.buffer[..n].to_vec())
    }

    /// A `std::vector<int>` option's values.
    pub fn ints(&self) -> &[i32] {
        match &self.value {
            Value::IntVec(v) => v,
            _ => &[],
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
        Kind::FloatVec => {
            // string_to_T(vector<float>&, s): split at ',' (':' when present), atof each piece
            let delin = if s.contains(':') { ':' } else { ',' };
            let mut rest = format!("{s}{delin}");
            let mut v = Vec::new();
            while !rest.is_empty() {
                let p = rest.find(delin).unwrap_or(rest.len());
                v.push(cnum::strtod(&rest[..p]).0 as f32);
                rest = rest[(p + 1).min(rest.len())..].to_owned();
            }
            (Value::FloatVec(v), true)
        }
        Kind::IntVec => {
            // string_to_T(vector<int>&, s): the same split, atoi each piece
            let delin = if s.contains(':') { ':' } else { ',' };
            let mut rest = format!("{s}{delin}");
            let mut v = Vec::new();
            while !rest.is_empty() {
                let p = rest.find(delin).unwrap_or(rest.len());
                v.push(cnum::atoi(&rest[..p]));
                rest = rest[(p + 1).min(rest.len())..].to_owned();
            }
            (Value::IntVec(v), true)
        }
    }
}

/// An `ifstream` read as `parse_config_file` reads it: words (`cf >> optstr`) and the rest of
/// a line (`cf.getline(buffer, 1024)`), with the stream's end-of-file and failure states.
struct ConfigReader<'a> {
    b: &'a [u8],
    pos: usize,
    eof: bool,
    fail: bool,
    /// Lines of any length (larmorx's reading; FSL's `getline(buffer, 1024)` fails the stream
    /// on a longer one).
    long_lines: bool,
    /// A line longer than `getline`'s 1023 characters was met with more text after it: FSL's
    /// reading ends there, before that text.
    cut: bool,
}

impl ConfigReader<'_> {
    /// `cf >> optstr`: `None` once the stream has failed (or stood at its end).
    fn word(&mut self) -> Option<String> {
        if self.fail || self.eof {
            self.fail = true;
            return None;
        }
        while self.pos < self.b.len() && is_space(self.b[self.pos]) {
            self.pos += 1;
        }
        if self.pos == self.b.len() {
            self.eof = true;
            self.fail = true;
            return None;
        }
        let start = self.pos;
        while self.pos < self.b.len() && !is_space(self.b[self.pos]) {
            self.pos += 1;
        }
        if self.pos == self.b.len() {
            self.eof = true;
        }
        Some(String::from_utf8_lossy(&self.b[start..self.pos]).into_owned())
    }

    /// `cf.getline(buffer, 1024)`: the line's rest up to its newline (taken, not kept), at most
    /// 1023 characters (more fail the stream), cut at a NUL as `string(buffer)` cuts it.
    fn getline(&mut self) -> String {
        if self.fail || self.eof {
            self.fail = true;
            return String::new();
        }
        let start = self.pos;
        let line_end = self.b[start..]
            .iter()
            .position(|&c| c == b'\n')
            .map_or(self.b.len(), |i| start + i);
        if line_end - start >= 1024 && self.b[line_end..].iter().skip(1).any(|&c| !is_space(c)) {
            // FSL's stream fails on this line: the text after it is never read
            self.cut = true;
        }
        let mut n = 0;
        while (self.long_lines || n + 1 < 1024)
            && self.pos < self.b.len()
            && self.b[self.pos] != b'\n'
        {
            self.pos += 1;
            n += 1;
        }
        let line = &self.b[start..self.pos];
        if self.pos == self.b.len() {
            self.eof = true;
            if n == 0 {
                self.fail = true;
            }
        } else if self.b[self.pos] == b'\n' {
            self.pos += 1;
        } else {
            self.fail = true;
        }
        let line = line.split(|&c| c == 0).next().unwrap_or(&[]);
        String::from_utf8_lossy(line).into_owned()
    }
}

/// `isspace` in the C locale.
fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
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
                buffer: match &o.default {
                    Value::FloatVec(v) => v.clone(),
                    _ => Vec::new(),
                },
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
            if let Value::FloatVec(new) = &v {
                // clear() keeps the buffer; push_back writes from its start
                for (k, x) in new.iter().enumerate() {
                    if k < st.buffer.len() {
                        st.buffer[k] = *x;
                    } else {
                        st.buffer.push(*x);
                    }
                }
            }
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

    /// `parse_config_file(filename)` after the command line (`parsed`): the options the file
    /// names, those already set left as they are; `Err` is the exception's text (an option that
    /// does not exist, a value that cannot be set, or a file that cannot be opened). A
    /// directory opens and reads as an empty file, as `ifstream` reads it.
    pub fn parse_config_file(&self, parsed: &mut Parsed, filename: &str) -> Result<(), String> {
        self.parse_config_file_lines(parsed, filename, false)
            .map(|_| ())
    }

    /// [`Self::parse_config_file`], with lines of any length when `long_lines` (FSL ends its
    /// reading at a line whose text after the first word is longer than 1023 characters).
    /// `Ok(true)`: such a line was met with more text after it (FSL ignores that text).
    pub fn parse_config_file_lines(
        &self,
        parsed: &mut Parsed,
        filename: &str,
        long_lines: bool,
    ) -> Result<bool, String> {
        let path = std::path::Path::new(filename);
        let bytes = if path.is_dir() {
            Vec::new()
        } else {
            std::fs::read(path).map_err(|_| format!("{filename}: Couldn't open the file!"))?
        };
        let mut cf = ConfigReader {
            b: &bytes,
            pos: 0,
            eof: false,
            fail: false,
            long_lines,
            cut: false,
        };
        while let Some(optstr) = cf.word() {
            if optstr.starts_with('#') {
                cf.getline();
            } else if optstr.starts_with("--") {
                let name = optstr.split('=').next().unwrap_or("");
                self.parse_option_ignore(&mut parsed.states, name, |p, st| {
                    p.parse_long_option(st, &optstr)
                })?;
            } else {
                let buffer = cf.getline();
                self.parse_option_ignore(&mut parsed.states, &optstr, |p, st| {
                    p.parse_option(st, &optstr, &buffer, &[], 0)
                })?;
            }
        }
        Ok(cf.cut)
    }

    /// `parse_option` in overwrite mode `Ignore`: an option that does not exist is an error,
    /// one already set is skipped, any other is parsed by `parse`.
    fn parse_option_ignore(
        &self,
        states: &mut [OptState],
        name: &str,
        parse: impl FnOnce(&Self, &mut [OptState]) -> Result<usize, String>,
    ) -> Result<(), String> {
        match self.opts.iter().position(|o| o.matches(name)) {
            None => Err(format!("{name}: Option doesn't exist!")),
            Some(i) if states[i].set => Ok(()),
            Some(_) => parse(self, states).map(|_| ()),
        }
    }

    /// `check_compulsory_arguments(true)`: the text it prints for the compulsory options that
    /// were not set, or `None` when every one was.
    pub fn check_compulsory(&self, parsed: &Parsed) -> Option<String> {
        let unset: Vec<&Opt> = self
            .opts
            .iter()
            .zip(&parsed.states)
            .filter(|(o, st)| o.compulsory && !st.set)
            .map(|(o, _)| o)
            .collect();
        if unset.is_empty() {
            return None;
        }
        let rule = "***************************************************\n";
        let mut s = format!("{rule}The following COMPULSORY options have not been set:\n");
        for o in unset {
            s.push_str(&o.usage_line());
            s.push('\n');
        }
        s.push_str(rule);
        Some(s)
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

    #[test]
    fn vector_values_and_compulsory() {
        let p = OptionParser::new(
            "T",
            "ex",
            vec![
                Opt {
                    key: "-t",
                    default: Value::Float(0.0),
                    help: "theta",
                    compulsory: true,
                    flag: ArgFlag::Requires(1),
                    visible: true,
                },
                Opt {
                    key: "-a,--axis",
                    default: Value::FloatVec(vec![0.0; 3]),
                    help: "~<a,b,c>\taxis",
                    compulsory: false,
                    flag: ArgFlag::Requires(1),
                    visible: true,
                },
            ],
        );
        let argv = |a: &[&str]| {
            let mut v = vec!["prog".to_owned()];
            v.extend(a.iter().map(|s| s.to_string()));
            v
        };
        let r = p
            .parse_command_line(&argv(&["-a", "1,x,,2.5"]), 0, false)
            .ok()
            .unwrap();
        assert_eq!(r.get(&p, "-a,--axis").floats(), [1.0, 0.0, 0.0, 2.5]);
        let r = p
            .parse_command_line(&argv(&["--axis=1:2,5:3"]), 0, false)
            .ok()
            .unwrap();
        assert_eq!(r.get(&p, "-a,--axis").floats(), [1.0, 2.0, 3.0]);
        // clear() keeps the buffer: past the new values, the earlier ones (or the default's)
        let r = p
            .parse_command_line(&argv(&["-a", "1,2,3,4", "-a", "7"]), 0, false)
            .ok()
            .unwrap();
        let a = r.get(&p, "-a,--axis");
        assert_eq!(a.floats(), [7.0]);
        assert_eq!(a.floats_buffer(3).unwrap(), [7.0, 2.0, 3.0]);
        let missing = p.check_compulsory(&r).unwrap();
        assert!(
            missing.contains("The following COMPULSORY options have not been set:\n\t-t\ttheta\n")
        );
        let r = p
            .parse_command_line(&argv(&["-t", "3"]), 0, false)
            .ok()
            .unwrap();
        assert!(p.check_compulsory(&r).is_none());
    }

    #[test]
    fn int_vectors_config_files_and_print() {
        let opt = |key, default, help| Opt {
            key,
            default,
            help,
            compulsory: false,
            flag: ArgFlag::Requires(1),
            visible: true,
        };
        let p = OptionParser::new(
            "T",
            "ex",
            vec![
                opt("--miter", Value::IntVec(vec![5]), "\titerations"),
                opt("--lambda", Value::FloatVec(vec![0.0]), "weights"),
                opt("--regmod", Value::Str("b".into()), "model"),
                opt("--n", Value::Int(1), "n"),
                Opt {
                    key: "-v,--verbose",
                    default: Value::Bool(false),
                    help: "verbose",
                    compulsory: false,
                    flag: ArgFlag::NoArgument,
                    visible: true,
                },
            ],
        );
        let argv = |a: &[&str]| {
            let mut v = vec!["prog".to_owned()];
            v.extend(a.iter().map(|s| s.to_string()));
            v
        };
        let mut r = p
            .parse_command_line(&argv(&["--miter=3,x,", "--n=7"]), 0, false)
            .ok()
            .unwrap();
        assert_eq!(r.get(&p, "--miter").ints(), [3, 0, 0]);
        let dir = tempfile::tempdir().unwrap();
        let cnf = dir.path().join("c.cnf");
        std::fs::write(
            &cnf,
            "# a comment\n--miter=5,5\n--lambda=0.0000005,1e-11\n  --n=2\n-v\n--regmod=m",
        )
        .unwrap();
        p.parse_config_file(&mut r, cnf.to_str().unwrap()).unwrap();
        // the command line's values are kept (overwrite mode Ignore)
        assert_eq!(r.get(&p, "--miter").ints(), [3, 0, 0]);
        assert_eq!(r.get(&p, "--n").int(), 7);
        assert_eq!(r.get(&p, "--lambda").floats(), [5e-7f32, 1e-11]);
        assert!(r.get(&p, "-v,--verbose").bool());
        assert_eq!(r.get(&p, "--regmod").str(), "m");
        let print = |k: &str| {
            let i = p.opts.iter().position(|o| o.key == k).unwrap();
            p.opts[i].print(&r.states[i])
        };
        assert_eq!(print("--miter"), "# \titerations\n--miter=3,0,0");
        assert_eq!(print("--lambda"), "# weights\n--lambda=5e-07,1e-11");
        assert_eq!(print("-v,--verbose"), "# verbose\n--verbose");
        // an unknown option, a file that cannot be opened, a line of 1024 characters
        std::fs::write(&cnf, "--nope=1\n").unwrap();
        assert_eq!(
            p.parse_config_file(&mut r, cnf.to_str().unwrap()),
            Err("--nope: Option doesn't exist!".to_owned())
        );
        let missing = dir.path().join("none.cnf");
        let m = missing.to_str().unwrap();
        assert_eq!(
            p.parse_config_file(&mut r, m),
            Err(format!("{m}: Couldn't open the file!"))
        );
        let mut r = p.parse_command_line(&argv(&[]), 0, false).ok().unwrap();
        std::fs::write(&cnf, format!("# {}\n--n=4\n", "x".repeat(1100))).unwrap();
        p.parse_config_file(&mut r, cnf.to_str().unwrap()).unwrap();
        assert_eq!(r.get(&p, "--n").int(), 1);
        // the same with lines of any length (larmorx's reading, topup's FSL-122): read on, and
        // the long line reported either way
        let c = cnf.to_str().unwrap();
        assert_eq!(p.parse_config_file_lines(&mut r, c, false), Ok(true));
        assert_eq!(r.get(&p, "--n").int(), 1);
        assert_eq!(p.parse_config_file_lines(&mut r, c, true), Ok(true));
        assert_eq!(r.get(&p, "--n").int(), 4);
        // 1023 characters fit; a long last line cuts nothing
        let mut r = p.parse_command_line(&argv(&[]), 0, false).ok().unwrap();
        std::fs::write(
            &cnf,
            format!("#{}\n--n=5\n# {}", " ".repeat(1023), "y ".repeat(600)),
        )
        .unwrap();
        assert_eq!(p.parse_config_file_lines(&mut r, c, false), Ok(false));
        assert_eq!(r.get(&p, "--n").int(), 5);
    }
}
