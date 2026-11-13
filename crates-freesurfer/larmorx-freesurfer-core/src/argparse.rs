// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's command-line parser of its newer programs (`ArgumentParser`, queue item 3.1-21,
//! FreeSurfer's own code derived from Hilton Bristow's argparse), translated from FreeSurfer
//! 8.2.0's `utils/argparse.cpp` and `include/argparse.h`: the arguments' configuration
//! (`addArgument`, `addHelp`, the default `--all-info` and `--version`), `parse` with its
//! messages and exit statuses, `exists` and `retrieve`. Programs reaching it: `AntsDenoiseImageFs`,
//! `AntsN4BiasFieldCorrectionFs`, `fs_temp_dir`, `fs_temp_file`, `mris_defects_pointset`,
//! `mris_remesh`, `mri_brainvol_stats`.
//!
//! The values are kept by type (`Value`); a `retrieve` of another type than the argument's is
//! FreeSurfer's `bad_cast` message. The index of names is an `unordered_map` in FreeSurfer, whose
//! order (libstdc++'s hash buckets, which the system's C++ library decides) chooses the first of
//! several names that strip to the same name (`unstrip`) and the name a duplicate's message
//! shows; the port keeps the names in the order they were added. No configuration of the seven
//! programs has two such names; command lines cannot make them except for programs with
//! positional arguments (none of the seven).
//!
//! Numbers are read as libstdc++'s `std::stoi` and `std::stof` read them (`strtol` and `strtof`
//! of the C library: leading white space, the longest number, `out_of_range` when `errno` is
//! `ERANGE`; `stof`'s NaN payloads as glibc's `nan(...)`), booleans as `TRUE`, `YES`, `1`,
//! `FALSE`, `NO`, `0` in any case. The help text is printed by FreeSurfer's XML help renderer
//! (`outputHelpXml`, `utils/fsPrintHelp.cpp`), which the caller passes in ([`HelpFn`]);
//! [`output_help_xml`] gives its two answers that need no XML and refuses the rest until that
//! renderer is ported.

use std::io::Write;

use larmorx_core::defects::FS_411;

use crate::cscan::{self, Bytes, Scan};
use crate::globals::Globals;
use crate::log::{Fatal, fatal};
use crate::run::Environment;
use crate::version::{get_all_info, get_version};

/// `ArgType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgType {
    Unknown,
    Bool,
    String,
    Int,
    Float,
}

/// The `nargs` of a variable count (`'+'`, `'*'`), as `char` values.
pub const PLUS: i8 = b'+' as i8;
pub const STAR: i8 = b'*' as i8;

/// One configured argument (`ArgumentParser::Argument`).
#[derive(Clone, Debug)]
struct Argument {
    short_name: String,
    name: String,
    argtype: ArgType,
    min_args: u32,
    consumed: u32,
    fixed: bool,
    required: bool,
    positional: bool,
    valid: bool,
    /// `fixed_nargs` (`size_t`) or, for a variable count, `variable_nargs` (`'+'`, `'*'`).
    nargs: u64,
}

impl Argument {
    /// `Argument()`: invalid, fixed with 0 inputs.
    fn invalid() -> Argument {
        Argument {
            short_name: String::new(),
            name: String::new(),
            argtype: ArgType::Unknown,
            min_args: 0,
            consumed: 0,
            fixed: true,
            required: false,
            positional: false,
            valid: false,
            nargs: 0,
        }
    }

    fn canonical_name(&self) -> &str {
        if self.name.is_empty() {
            &self.short_name
        } else {
            &self.name
        }
    }

    fn type_name(&self) -> &'static str {
        match self.argtype {
            ArgType::Int => "int",
            ArgType::Float => "float",
            ArgType::Bool => "bool",
            ArgType::String => "std::string",
            ArgType::Unknown => "unknown",
        }
    }

    /// `Argument(short_name, name, nargs, argtype, required)`.
    fn new(
        g: &mut Globals,
        err: &mut dyn Write,
        short_name: &str,
        name: &str,
        nargs: i8,
        argtype: ArgType,
        required: bool,
    ) -> Result<Argument, Fatal> {
        let mut a = Argument {
            short_name: short_name.to_owned(),
            name: name.to_owned(),
            argtype,
            min_args: 0,
            consumed: 0,
            fixed: true,
            required,
            positional: false,
            valid: true,
            nargs: 0,
        };
        let mut nargs = nargs;
        // `name[0]` of an empty name is its terminating NUL: a short-only flag is positional
        if name.as_bytes().first() != Some(&b'-') {
            a.positional = true;
            a.required = true;
            if nargs == 0 {
                nargs = 1;
            }
            if nargs == STAR {
                nargs = PLUS;
            }
        }
        if nargs == PLUS || nargs == STAR {
            a.nargs = nargs as u64;
            a.min_args = u32::from(nargs == PLUS);
            a.fixed = false;
        } else {
            a.nargs = i64::from(nargs) as u64;
            a.min_args = i32::from(nargs) as u32;
            a.fixed = true;
        }
        if a.argtype == ArgType::Unknown {
            a.argtype = if a.min_args == 0 {
                ArgType::Bool
            } else {
                ArgType::String
            };
        }
        if a.min_args == 0 && a.argtype != ArgType::Bool {
            fatal(
                g,
                err,
                1,
                &format!(
                    "invalid argument configuration for '{}'. Option flags that accept no input must be of type ArgType::Bool",
                    a.canonical_name()
                ),
            )?;
        }
        if a.min_args == 0 && a.required {
            fatal(
                g,
                err,
                1,
                &format!(
                    "invalid argument configuration for '{}'. Required flags must accept at least one input",
                    a.canonical_name()
                ),
            )?;
        }
        Ok(a)
    }

    /// `validate()`: the inputs counted against the configuration (status 2).
    fn validate(&self, g: &mut Globals, err: &mut dyn Write) -> Result<(), Fatal> {
        if self.positional && self.consumed < self.min_args {
            fatal(g, err, 2, "not enough positional arguments supplied")?;
        }
        if self.fixed && self.nargs != u64::from(self.consumed) {
            fatal(
                g,
                err,
                2,
                &format!(
                    "not enough inputs passed to option '{}' (expected {})",
                    self.canonical_name(),
                    self.nargs
                ),
            )?;
        }
        if !self.fixed && self.nargs == PLUS as u64 && self.consumed < 1 {
            fatal(
                g,
                err,
                2,
                &format!(
                    "option '{}' requires at least one input",
                    self.canonical_name()
                ),
            )?;
        }
        Ok(())
    }

    /// One value (`fixed_nargs <= 1` and fixed) or a list.
    fn scalar(&self) -> bool {
        self.fixed && self.nargs <= 1
    }
}

/// A parsed value (`Any`'s held types).
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Int(i32),
    Float(f32),
    Bool(bool),
    Str(String),
    Ints(Vec<i32>),
    Floats(Vec<f32>),
    Bools(Vec<bool>),
    Strs(Vec<String>),
}

impl Value {
    /// The C++ type `retrieve` must name.
    pub fn cpp_type(&self) -> &'static str {
        match self {
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Bool(_) => "bool",
            Value::Str(_) => "std::string",
            Value::Ints(_) => "std::vector<int>",
            Value::Floats(_) => "std::vector<float>",
            Value::Bools(_) => "std::vector<bool>",
            Value::Strs(_) => "std::vector<std::string>",
        }
    }
}

/// The help renderer `parse` calls (`outputHelpXml(text, size)`: the text, or `None` for NULL;
/// standard output, standard error).
pub type HelpFn = fn(&mut Globals, &mut dyn Write, &mut dyn Write, Option<&[u8]>) -> i32;

/// `outputHelpXml(text, size)` for what needs no XML: no text, `No xml string passed.` (standard
/// error, -1); an empty one, `Document not parsed successfully. ` (libxml2 makes no document of 0
/// bytes; -1). A text to render: not ported yet (a message, -1).
pub fn output_help_xml(
    _g: &mut Globals,
    _out: &mut dyn Write,
    err: &mut dyn Write,
    text: Option<&[u8]>,
) -> i32 {
    match text {
        None => {
            let _ = err.write_all(b"No xml string passed.\n");
        }
        Some([]) => {
            let _ = err.write_all(b"Document not parsed successfully. \n");
        }
        Some(_) => {
            let _ = err.write_all(
                b"larmorx-freesurfer: the XML help renderer (fsPrintHelp) is not ported yet\n",
            );
        }
    }
    -1
}

/// `ArgumentParser`.
#[derive(Clone, Debug)]
pub struct ArgumentParser {
    /// The names (`IndexMap`), in the order they were added (see the module's notes).
    index: Vec<(String, usize)>,
    variable_positional: bool,
    variable_flag: bool,
    app_name: String,
    arguments: Vec<Argument>,
    positionals: Vec<Argument>,
    variables: Vec<(bool, Value)>,
    /// `helptext` (`None`: never set, FS-411) and `helptextsize`.
    helptext: Option<Vec<u8>>,
    helptextsize: u32,
}

/// `strip(name)`: one leading dash removed, and a second when the name is longer than 3 bytes.
fn strip(name: &str) -> &str {
    let b = name.as_bytes();
    let mut begin = 0;
    begin += usize::from(!b.is_empty() && b[0] == b'-');
    begin += usize::from(b.len() > 3 && b[1] == b'-');
    &name[begin..]
}

/// `verifyOption(name)` (status 1 for a bad one).
fn verify_option<'a>(
    g: &mut Globals,
    err: &mut dyn Write,
    name: &'a str,
) -> Result<&'a str, Fatal> {
    let b = name.as_bytes();
    if b.is_empty() {
        fatal(
            g,
            err,
            1,
            "invalid argument configuration. Argument names must not be empty",
        )?;
    }
    if (b.len() == 2 && b[0] != b'-') || b.len() == 3 {
        fatal(
            g,
            err,
            1,
            &format!(
                "invalid argument configuration for '{name}'. Short names must begin with '-'"
            ),
        )?;
    }
    if b.len() > 3 && (b[0] != b'-' || b[1] != b'-') {
        fatal(
            g,
            err,
            1,
            &format!(
                "invalid argument configuration for '{name}'. Multi-character names must begin with '--'"
            ),
        )?;
    }
    Ok(name)
}

/// `stob(str)`: `TRUE`, `YES`, `1` / `FALSE`, `NO`, `0` in any case (the C locale's `toupper`).
fn stob(s: &str) -> Option<bool> {
    let u = s.to_ascii_uppercase();
    match u.as_str() {
        "TRUE" | "YES" | "1" => Some(true),
        "FALSE" | "NO" | "0" => Some(false),
        _ => None,
    }
}

/// `std::stoi(s)`: `strtol(s, &end, 10)`; `None` (an exception) when no digits are read, when
/// `errno` is `ERANGE` or the `long` is outside the `int` range.
pub fn stoi(s: &str) -> Option<i32> {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && crate::cstd::is_space(b[i]) {
        i += 1;
    }
    let neg = i < b.len() && b[i] == b'-';
    if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
        i += 1;
    }
    let start = i;
    let mut v: i128 = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        v = (v * 10 + i128::from(b[i] - b'0')).min(i128::from(i64::MAX) + 2);
        i += 1;
    }
    if i == start {
        return None;
    }
    let v = if neg { -v } else { v };
    i32::try_from(v).ok()
}

/// The longest prefix of `b` from `i` that `strtod` reads as a number (white space and sign
/// included), and the place of a `nan(...)`'s character sequence.
fn strtod_prefix(b: &[u8]) -> (usize, Option<(usize, usize)>) {
    let mut i = 0;
    while i < b.len() && crate::cstd::is_space(b[i]) {
        i += 1;
    }
    if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
        i += 1;
    }
    let rest = &b[i..];
    let starts = |w: &[u8]| rest.len() >= w.len() && rest[..w.len()].eq_ignore_ascii_case(w);
    if starts(b"infinity") {
        return (i + 8, None);
    }
    if starts(b"inf") {
        return (i + 3, None);
    }
    if starts(b"nan") {
        let j = i + 3;
        if j < b.len() && b[j] == b'(' {
            let mut k = j + 1;
            while k < b.len() && (b[k].is_ascii_alphanumeric() || b[k] == b'_') {
                k += 1;
            }
            if k < b.len() && b[k] == b')' {
                return (k + 1, Some((j + 1, k)));
            }
        }
        return (j, None);
    }
    let digits = |mut k: usize, hex: bool| {
        while k < b.len() && (b[k].is_ascii_digit() || (hex && b[k].is_ascii_hexdigit())) {
            k += 1;
        }
        k
    };
    let hex = rest.len() >= 2 && rest[0] == b'0' && (rest[1] == b'x' || rest[1] == b'X');
    if hex {
        let s = i + 2;
        let mut k = digits(s, true);
        let mut n = k - s;
        if k < b.len() && b[k] == b'.' {
            let f = digits(k + 1, true);
            n += f - (k + 1);
            k = f;
        }
        if n == 0 {
            // "0x" without hexadecimal digits: the number is the 0
            return (i + 1, None);
        }
        if k < b.len() && (b[k] == b'p' || b[k] == b'P') {
            let mut e = k + 1;
            if e < b.len() && (b[e] == b'-' || b[e] == b'+') {
                e += 1;
            }
            let d = digits(e, false);
            if d > e {
                k = d;
            }
        }
        return (k, None);
    }
    let mut k = digits(i, false);
    let mut n = k - i;
    if k < b.len() && b[k] == b'.' {
        let f = digits(k + 1, false);
        n += f - (k + 1);
        k = f;
    }
    if n == 0 {
        return (0, None);
    }
    if k < b.len() && (b[k] == b'e' || b[k] == b'E') {
        let mut e = k + 1;
        if e < b.len() && (b[e] == b'-' || b[e] == b'+') {
            e += 1;
        }
        let d = digits(e, false);
        if d > e {
            k = d;
        }
    }
    (k, None)
}

/// `strtoull(s, &end, 0)` of a whole `nan(...)` sequence (`None` unless every byte is read).
fn strtoull0(s: &[u8]) -> Option<u64> {
    let (base, digits) = if s.len() >= 2 && s[0] == b'0' && (s[1] | 0x20) == b'x' {
        (16u32, &s[2..])
    } else if !s.is_empty() && s[0] == b'0' {
        (8, s)
    } else {
        (10, s)
    };
    if digits.is_empty() {
        // "0x" alone reads the 0 and stops before the x
        return if s.len() == 2 { None } else { Some(0) };
    }
    let mut v: u64 = 0;
    let mut over = false;
    for &c in digits {
        let d = (c as char).to_digit(base)?;
        match v
            .checked_mul(u64::from(base))
            .and_then(|x| x.checked_add(u64::from(d)))
        {
            Some(x) => v = x,
            None => over = true,
        }
    }
    Some(if over { u64::MAX } else { v })
}

/// `std::stof(s)`: `strtof(s, &end)`; `None` (an exception) when no number is read or `errno` is
/// `ERANGE` (overflow, and a tiny inexact result). `nan(seq)` takes `strtoull(seq, 0)`'s low 22
/// bits as its payload when they are not all 0, as glibc does.
pub fn stof(s: &str) -> Option<f32> {
    let b = s.as_bytes();
    let (end, seq) = strtod_prefix(b);
    if end == 0 {
        return None;
    }
    let mut st = Bytes::new(&b[..end]);
    match cscan::scan_f32(&mut st) {
        Scan::Value(v, erange) => {
            if erange {
                return None;
            }
            if let (true, Some((a, z))) = (v.is_nan(), seq)
                && let Some(m) = strtoull0(&b[a..z])
            {
                let payload = (m as u32) & 0x3f_ffff;
                if payload != 0 {
                    return Some(f32::from_bits((v.to_bits() & 0xffc0_0000) | payload));
                }
            }
            Some(v)
        }
        Scan::Fail | Scan::Eof => None,
    }
}

impl ArgumentParser {
    /// `ArgumentParser()`: the default `--all-info` and `--version` flags.
    pub fn new(g: &mut Globals, err: &mut dyn Write) -> Result<ArgumentParser, Fatal> {
        let mut p = ArgumentParser {
            index: Vec::new(),
            variable_positional: false,
            variable_flag: false,
            app_name: String::new(),
            arguments: Vec::new(),
            positionals: Vec::new(),
            variables: Vec::new(),
            helptext: None,
            helptextsize: 0,
        };
        p.add_argument(g, err, "--all-info", 0, ArgType::Bool, false)?;
        p.add_argument(g, err, "--version", 0, ArgType::Bool, false)?;
        Ok(p)
    }

    fn find(&self, key: &str) -> Option<usize> {
        self.index.iter().find(|(k, _)| k == key).map(|&(_, n)| n)
    }

    /// `index[key]` (`operator[]`: an unknown key is added with 0).
    fn at(&mut self, key: &str) -> usize {
        match self.find(key) {
            Some(n) => n,
            None => {
                self.index.push((key.to_owned(), 0));
                0
            }
        }
    }

    /// `addArgument(name, nargs, argtype, required)`: a positional argument for a name without a
    /// leading dash, else a flag (a short one for names of 2 bytes or fewer).
    pub fn add_argument(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
        name: &str,
        nargs: i8,
        argtype: ArgType,
        required: bool,
    ) -> Result<(), Fatal> {
        let arg = if name.as_bytes().first() != Some(&b'-') {
            Argument::new(g, err, "", name, nargs, argtype, true)?
        } else if name.len() > 2 {
            let n = verify_option(g, err, name)?;
            Argument::new(g, err, "", n, nargs, argtype, required)?
        } else {
            let n = verify_option(g, err, name)?;
            Argument::new(g, err, n, "", nargs, argtype, required)?
        };
        self.insert_argument(g, err, arg)
    }

    /// `addArgument(short_name, name, nargs, argtype, required)`.
    #[allow(clippy::too_many_arguments)]
    pub fn add_argument2(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
        short_name: &str,
        name: &str,
        nargs: i8,
        argtype: ArgType,
        required: bool,
    ) -> Result<(), Fatal> {
        let s = verify_option(g, err, short_name)?;
        let n = verify_option(g, err, name)?;
        let arg = Argument::new(g, err, s, n, nargs, argtype, required)?;
        self.insert_argument(g, err, arg)
    }

    /// `addHelp(text, size)`: the help text and a `-h`/`--help` flag.
    pub fn add_help(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
        text: &[u8],
    ) -> Result<(), Fatal> {
        self.helptext = Some(text.to_vec());
        self.helptextsize = text.len() as u32;
        self.add_argument2(g, err, "-h", "--help", 0, ArgType::Bool, false)
    }

    /// `insertArgument(arg)`.
    fn insert_argument(
        &mut self,
        g: &mut Globals,
        err: &mut dyn Write,
        arg: Argument,
    ) -> Result<(), Fatal> {
        let n = self.arguments.len();
        self.arguments.push(arg.clone());
        if arg.positional {
            self.positionals.push(arg.clone());
        }
        let v = match (arg.scalar(), arg.argtype) {
            (true, ArgType::Int) => Value::Int(0),
            (true, ArgType::Float) => Value::Float(0.0),
            (true, ArgType::Bool) => Value::Bool(false),
            (true, ArgType::String) => Value::Str(String::new()),
            (false, ArgType::Int) => Value::Ints(Vec::new()),
            (false, ArgType::Float) => Value::Floats(Vec::new()),
            (false, ArgType::Bool) => Value::Bools(Vec::new()),
            (false, ArgType::String) => Value::Strs(Vec::new()),
            (_, ArgType::Unknown) => {
                fatal(
                    g,
                    err,
                    1,
                    &format!("unknown argument type for '{}'", arg.canonical_name()),
                )?;
                Value::Bool(false)
            }
        };
        self.variables.push((false, v));
        let dup = self
            .index
            .iter()
            .map(|(k, _)| strip(k))
            .find(|s| *s == strip(&arg.short_name) || *s == strip(&arg.name))
            .map(str::to_owned);
        if let Some(stripped) = dup {
            fatal(
                g,
                err,
                1,
                &format!(
                    "invalid argument configuration. Short name '{stripped}' of '{}' is used twice",
                    arg.canonical_name()
                ),
            )?;
        }
        if !arg.fixed {
            if arg.positional {
                if self.variable_positional {
                    fatal(
                        g,
                        err,
                        1,
                        &format!(
                            "invalid argument configuration for '{}'. Two positional arguments cannot both have a variable amount of inputs, as this could lead to an undefined boundary between the two",
                            arg.canonical_name()
                        ),
                    )?;
                }
                self.variable_positional = true;
            } else {
                self.variable_flag = true;
            }
            if self.variable_positional && self.variable_flag {
                fatal(
                    g,
                    err,
                    1,
                    &format!(
                        "invalid argument configuration for '{}'. A positional argument and flagged argument cannot both have a variable amount of inputs, as this could lead to an undefined boundary between the two",
                        arg.canonical_name()
                    ),
                )?;
            }
        }
        for key in [&arg.short_name, &arg.name] {
            if !key.is_empty() {
                match self.index.iter_mut().find(|(k, _)| k == key) {
                    Some(e) => e.1 = n,
                    None => self.index.push((key.clone(), n)),
                }
            }
        }
        Ok(())
    }

    /// `unstrip(name)`: the first configured name that strips to `name`, else `name`.
    fn unstrip(&self, name: &str) -> String {
        self.index
            .iter()
            .find(|(k, _)| strip(k) == name)
            .map_or_else(|| name.to_owned(), |(k, _)| k.clone())
    }

    /// `exists(name)`: whether the argument was given (`name` with or without its dashes); an
    /// unknown name ends the program (`'<name>' is not a known argument`, status 1).
    pub fn exists(&self, g: &mut Globals, err: &mut dyn Write, name: &str) -> Result<bool, Fatal> {
        let u = self.unstrip(name);
        let Some(n) = self.find(&u) else {
            fatal(g, err, 1, &format!("'{u}' is not a known argument"))?;
            return Ok(false);
        };
        Ok(self.variables[n].0)
    }

    /// `retrieve<T>(name)` with `T` named by `want` ([`Value::cpp_type`]): the value; an unknown
    /// name or another type ends the program with FreeSurfer's messages (status 1).
    pub fn retrieve(
        &self,
        g: &mut Globals,
        err: &mut dyn Write,
        name: &str,
        want: &str,
    ) -> Result<Value, Fatal> {
        let u = self.unstrip(name);
        let Some(n) = self.find(&u) else {
            fatal(g, err, 1, &format!("'{u}' is not a known argument"))?;
            return Ok(Value::Bool(false));
        };
        let v = &self.variables[n].1;
        if v.cpp_type() == want {
            return Ok(v.clone());
        }
        let a = &self.arguments[n];
        let (fulltype, starter) = if a.scalar() {
            (a.type_name().to_owned(), "This input is")
        } else {
            (
                format!("std::vector<{}>", a.type_name()),
                "These inputs are",
            )
        };
        fatal(
            g,
            err,
            1,
            &format!(
                "invalid cast of argument '{name}'. {starter} of type '{}' and should be retrieved via retrieve<{fulltype}>(\"{name}\"). To change the expected type, modify the call to addArgument()",
                a.type_name()
            ),
        )?;
        Ok(v.clone())
    }

    /// `parse(argc, argv)` (`argv[0]` the program's name): the inputs checked and read; then
    /// `--version` (`<basename> freesurfer <version>` on standard output) and `--all-info`, which
    /// end the program (status 0) when they are its only arguments; `--help` (with a help text:
    /// `help`'s rendering, status 0); a required argument not given: `help` of the text (FS-411
    /// without one), then `missing required input '<name>'` (status 2).
    #[allow(clippy::too_many_arguments)]
    pub fn parse(
        &mut self,
        g: &mut Globals,
        env: &Environment,
        out: &mut dyn Write,
        err: &mut dyn Write,
        av: &[String],
        help: HelpFn,
    ) -> Result<(), Fatal> {
        // the program's name: the last of av[0]'s '/'-separated parts (empty after a final '/')
        let progname = match av.first() {
            Some(a0) if !a0.is_empty() => a0.rsplit('/').next().unwrap_or("").to_owned(),
            _ => String::new(),
        };
        if let Some(a0) = av.first() {
            self.app_name = a0.clone();
        }
        let argv = av;
        for (i, element) in argv.iter().enumerate().skip(1) {
            if let Some(n) = self.find(element) {
                let following = argv[i + 1..]
                    .iter()
                    .filter(|f| self.find(f).is_none())
                    .count() as u64;
                if u64::from(self.arguments[n].min_args) > following {
                    fatal(
                        g,
                        err,
                        2,
                        &format!("not enough inputs supplied to '{element}'"),
                    )?;
                }
            }
        }
        let mut posidx: u32 = u32::MAX;
        let mut active = Argument::invalid();
        for (i, element) in argv.iter().enumerate().skip(1) {
            if self.find(element).is_none() {
                if !active.valid && !self.positionals.is_empty() {
                    posidx = posidx.wrapping_add(1);
                    active = self.positionals[posidx as usize].clone();
                }
                if active.fixed && active.nargs == u64::from(active.consumed) {
                    posidx = posidx.wrapping_add(1);
                    if posidx as usize >= self.positionals.len() {
                        fatal(g, err, 2, &format!("unexpected argument '{element}'"))?;
                        return Ok(());
                    }
                    active = self.positionals[posidx as usize].clone();
                } else if !active.fixed && active.consumed >= active.min_args {
                    let minimum: i64 = self
                        .positionals
                        .iter()
                        .skip(posidx.wrapping_add(1) as usize)
                        .map(|p| i64::from(p.min_args as i32))
                        .sum();
                    if minimum > 0 {
                        let mut remaining: i64 = 0;
                        for future in &argv[i + 1..] {
                            if future.as_bytes().first() == Some(&b'-') {
                                let n = self.at(future);
                                remaining -= i64::from(self.arguments[n].min_args as i32);
                            } else {
                                remaining += 1;
                            }
                        }
                        if minimum > remaining {
                            posidx = posidx.wrapping_add(1);
                            active = self.positionals[posidx as usize].clone();
                        }
                    }
                }
                let n = self.at(active.canonical_name());
                let bad = || {
                    format!(
                        "input '{element}' cannot be converted to expected type ({})",
                        active.type_name()
                    )
                };
                let ok = if active.scalar() {
                    match active.argtype {
                        ArgType::Int => stoi(element).map(Value::Int),
                        ArgType::Float => stof(element).map(Value::Float),
                        ArgType::Bool => stob(element).map(Value::Bool),
                        _ => Some(Value::Str(element.clone())),
                    }
                    .map(|v| self.variables[n].1 = v)
                } else {
                    let slot = &mut self.variables[n].1;
                    match (active.argtype, slot) {
                        (ArgType::Int, Value::Ints(l)) => stoi(element).map(|v| l.push(v)),
                        (ArgType::Float, Value::Floats(l)) => stof(element).map(|v| l.push(v)),
                        (ArgType::Bool, Value::Bools(l)) => stob(element).map(|v| l.push(v)),
                        (_, Value::Strs(l)) => {
                            l.push(element.clone());
                            Some(())
                        }
                        _ => None,
                    }
                };
                if ok.is_none() {
                    fatal(g, err, 2, &bad())?;
                }
                self.variables[n].0 = true;
                active.consumed += 1;
            } else {
                if active.valid {
                    active.validate(g, err)?;
                }
                let n = self.at(element);
                active = self.arguments[n].clone();
                if active.fixed && active.nargs == 0 {
                    let n = self.at(active.canonical_name());
                    self.variables[n] = (true, Value::Bool(true));
                }
            }
        }
        if active.valid {
            active.validate(g, err)?;
        }
        let mut info = 0usize;
        if self.exists(g, err, "version")? {
            let _ = writeln!(out, "{progname} freesurfer {}", get_version());
            info += 1;
        }
        if self.exists(g, err, "all-info")? {
            let _ = writeln!(out, "{}", get_all_info(env, av, &progname));
            info += 1;
        }
        if info > 0 && av.len().wrapping_sub(info) == 1 {
            return Err(Fatal::Exit(0));
        }
        if self.helptextsize > 0 && self.exists(g, err, "help")? {
            help(g, out, err, self.helptext.as_deref());
            return Err(Fatal::Exit(0));
        }
        for k in 0..self.arguments.len() {
            let a = self.arguments[k].clone();
            if a.required && !self.exists(g, err, a.canonical_name())? {
                if self.helptext.is_none() {
                    g.notices.add(
                        &FS_411,
                        "a required input missing in a program without help text: FreeSurfer's help pointer is memory never written (0 taken)",
                    );
                }
                help(g, out, err, self.helptext.as_deref());
                fatal(
                    g,
                    err,
                    2,
                    &format!("missing required input '{}'", a.canonical_name()),
                )?;
            }
        }
        Ok(())
    }
}
