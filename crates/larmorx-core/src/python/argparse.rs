// SPDX-License-Identifier: Apache-2.0 AND PSF-2.0
//! `argparse.ArgumentParser` (CPython 3.12.16) for programs whose arguments are all options:
//! parsing (`parse_args`), the usage and help text (`HelpFormatter`, and
//! `ArgumentDefaultsHelpFormatter`'s "(default: ...)"), and every message and exit status
//! (module documentation in [`super`]).
//!
//! A parse either gives the values ([`Namespace`]) or an [`Exit`]: what the program prints on
//! its standard output and error and the status it exits with (0 after `-h` or `--version`, 2
//! after a usage error: the usage, then `<prog>: error: <message>`). Actions run in the order
//! argparse runs them, so the first error met is the one reported.

use std::collections::HashSet;

use super::number::{float_repr, parse_float, parse_int};
use super::textwrap;

/// A value in the namespace (Python's `None`, `bool`, `str`, `int`, `float` or `list`).
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    None,
    Bool(bool),
    Str(String),
    Int(i128),
    Float(f64),
    List(Vec<Value>),
}

impl Value {
    /// Python's `str(value)`.
    pub fn py_str(&self) -> String {
        match self {
            Value::None => "None".into(),
            Value::Bool(true) => "True".into(),
            Value::Bool(false) => "False".into(),
            Value::Str(s) => s.clone(),
            Value::Int(i) => i.to_string(),
            Value::Float(f) => float_repr(*f),
            Value::List(v) => format!(
                "[{}]",
                v.iter().map(Value::py_repr).collect::<Vec<_>>().join(", ")
            ),
        }
    }

    /// Python's `repr(value)`.
    pub fn py_repr(&self) -> String {
        match self {
            Value::Str(s) => py_repr_str(s),
            other => other.py_str(),
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> bool {
        matches!(self, Value::Bool(true))
    }

    pub fn as_list(&self) -> Option<&[Value]> {
        match self {
            Value::List(v) => Some(v),
            _ => None,
        }
    }
}

/// Python's `repr(str)`: single quotes unless the text holds one and no double quote; `\\`,
/// the quote, `\t`, `\n`, `\r` and other control characters escaped (`\xNN`, `\uNNNN`).
/// Unicode's non-printable categories beyond the control characters are not escaped (a
/// documented approximation).
pub fn py_repr_str(s: &str) -> String {
    let q = if s.contains('\'') && !s.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(q);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c if c == q => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || (0x7f..0xa0).contains(&(c as u32)) => {
                out.push_str(&format!("\\x{:02x}", c as u32));
            }
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push(q);
    out
}

/// How many command-line strings an option takes (`nargs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nargs {
    /// `None`: exactly one.
    Single,
    /// `'?'`.
    Optional,
    /// `'*'`.
    ZeroOrMore,
    /// `'+'`.
    OneOrMore,
    /// An integer (`0` for flags).
    Exactly(usize),
}

/// The action an option takes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Store,
    StoreTrue,
    StoreFalse,
    Help,
    /// `action="version"` with its text.
    Version(String),
}

/// The result of an option's `type` function on one string.
#[derive(Clone, Debug, PartialEq)]
pub enum Conversion {
    Ok(Value),
    /// A `ValueError` or `TypeError`: "invalid <type> value: '<string>'".
    Invalid,
    /// An `ArgumentTypeError` with its message.
    TypeError(String),
    /// The function called `parser.error(message)` itself (as tedana's `is_valid_file` does).
    ParserError(String),
}

/// An option's `type`.
pub enum Type {
    Str,
    Int,
    Float,
    /// A function, with the `__name__` argparse prints (`<lambda>` for a lambda).
    Custom(String, Box<dyn Fn(&str) -> Conversion + Send + Sync>),
}

impl Type {
    fn convert(&self, s: &str) -> Conversion {
        match self {
            Type::Str => Conversion::Ok(Value::Str(s.to_owned())),
            Type::Int => {
                parse_int(s).map_or(Conversion::Invalid, |v| Conversion::Ok(Value::Int(v)))
            }
            Type::Float => {
                parse_float(s).map_or(Conversion::Invalid, |v| Conversion::Ok(Value::Float(v)))
            }
            Type::Custom(_, f) => f(s),
        }
    }

    fn name(&self) -> &str {
        match self {
            Type::Str => "str",
            Type::Int => "int",
            Type::Float => "float",
            Type::Custom(n, _) => n,
        }
    }
}

/// One option (`parser.add_argument(...)`).
pub struct Argument {
    option_strings: Vec<String>,
    dest: Option<String>,
    nargs: Nargs,
    kind: Kind,
    metavar: Option<String>,
    choices: Option<Vec<String>>,
    required: bool,
    /// `None`: no help; `Some(None)`: `argparse.SUPPRESS`.
    help: Option<Option<String>>,
    /// `None`: `argparse.SUPPRESS`.
    default: Option<Value>,
    ty: Type,
}

impl Argument {
    /// A `store` option with `dest`, one value, no default (`None`), type `str`.
    pub fn store(option_strings: &[&str], dest: &str) -> Argument {
        Argument {
            option_strings: option_strings.iter().map(|s| (*s).to_owned()).collect(),
            dest: Some(dest.to_owned()),
            nargs: Nargs::Single,
            kind: Kind::Store,
            metavar: None,
            choices: None,
            required: false,
            help: None,
            default: Some(Value::None),
            ty: Type::Str,
        }
    }

    /// `action="store_true"` (default `False`).
    pub fn store_true(option_strings: &[&str], dest: &str) -> Argument {
        Argument {
            nargs: Nargs::Exactly(0),
            kind: Kind::StoreTrue,
            default: Some(Value::Bool(false)),
            ..Argument::store(option_strings, dest)
        }
    }

    /// `action="version"`: dest and default suppressed, the standard help.
    pub fn version(option_strings: &[&str], text: &str) -> Argument {
        Argument {
            dest: None,
            nargs: Nargs::Exactly(0),
            kind: Kind::Version(text.to_owned()),
            default: None,
            help: Some(Some("show program's version number and exit".into())),
            ..Argument::store(option_strings, "")
        }
    }

    fn help_action() -> Argument {
        Argument {
            dest: None,
            nargs: Nargs::Exactly(0),
            kind: Kind::Help,
            default: None,
            help: Some(Some("show this help message and exit".into())),
            ..Argument::store(&["-h", "--help"], "")
        }
    }

    pub fn nargs(mut self, nargs: Nargs) -> Self {
        self.nargs = nargs;
        self
    }
    pub fn metavar(mut self, m: &str) -> Self {
        self.metavar = Some(m.to_owned());
        self
    }
    pub fn choices(mut self, c: &[&str]) -> Self {
        self.choices = Some(c.iter().map(|s| (*s).to_owned()).collect());
        self
    }
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }
    pub fn help(mut self, h: &str) -> Self {
        self.help = Some(Some(h.to_owned()));
        self
    }
    /// `help=argparse.SUPPRESS`.
    pub fn hidden(mut self) -> Self {
        self.help = Some(None);
        self
    }
    pub fn default(mut self, v: Value) -> Self {
        self.default = Some(v);
        self
    }
    pub fn ty(mut self, ty: Type) -> Self {
        self.ty = ty;
        self
    }

    fn suppressed(&self) -> bool {
        matches!(self.help, Some(None))
    }

    /// `_get_action_name`.
    fn name(&self) -> String {
        self.option_strings.join("/")
    }
}

/// The parsed values, in the order the options were added.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Namespace(pub Vec<(String, Value)>);

impl Namespace {
    pub fn get(&self, dest: &str) -> &Value {
        self.0
            .iter()
            .find(|(d, _)| d == dest)
            .map_or(&Value::None, |(_, v)| v)
    }

    fn set(&mut self, dest: &str, v: Value) {
        match self.0.iter_mut().find(|(d, _)| d == dest) {
            Some(slot) => slot.1 = v,
            None => self.0.push((dest.to_owned(), v)),
        }
    }

    fn has(&self, dest: &str) -> bool {
        self.0.iter().any(|(d, _)| d == dest)
    }
}

/// What the program prints and its exit status, when parsing ends the program.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Exit {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

/// The width argparse's formatter uses: `shutil.get_terminal_size().columns - 2`, from
/// `COLUMNS` (Python's `int()` of it, if positive), else the terminal's width of the standard
/// output when it is one (`terminal_columns`), else 80.
pub fn formatter_width(columns_env: Option<&str>, terminal_columns: Option<usize>) -> isize {
    let env = columns_env.and_then(parse_int).filter(|&c| c > 0);
    let cols = match env {
        Some(c) => c.min(isize::MAX as i128 / 2) as isize,
        None => terminal_columns.filter(|&c| c > 0).unwrap_or(80) as isize,
    };
    cols - 2
}

struct OptTuple {
    action: Option<usize>,
    option_string: String,
    /// `None`, `Some("")` (a cluster's tail) or `Some("=")`.
    sep: Option<String>,
    explicit: Option<String>,
}

enum Stop {
    Error(Option<usize>, String),
    Exit(Exit),
}

/// An argument parser with options only (module documentation).
pub struct Parser {
    prog: String,
    defaults_help: bool,
    actions: Vec<Argument>,
    /// The action groups: "positional arguments", "options", then the program's own.
    groups: Vec<(String, Vec<usize>)>,
}

impl Parser {
    /// `ArgumentParser(prog=prog)`, with `formatter_class=ArgumentDefaultsHelpFormatter` when
    /// `defaults_help`.
    pub fn new(prog: &str, defaults_help: bool) -> Parser {
        let mut p = Parser {
            prog: prog.to_owned(),
            defaults_help,
            actions: Vec::new(),
            groups: vec![
                ("positional arguments".into(), Vec::new()),
                ("options".into(), Vec::new()),
            ],
        };
        p.add(None, Argument::help_action());
        p
    }

    /// `parser.add_argument_group(title)`; returns the group to add options to.
    pub fn add_group(&mut self, title: &str) -> usize {
        self.groups.push((title.to_owned(), Vec::new()));
        self.groups.len() - 1
    }

    /// Adds an option to `group` (`None`: the parser itself, the "options" group).
    pub fn add(&mut self, group: Option<usize>, arg: Argument) {
        self.actions.push(arg);
        let i = self.actions.len() - 1;
        self.groups[group.unwrap_or(1)].1.push(i);
    }

    pub fn prog(&self) -> &str {
        &self.prog
    }

    fn option_strings(&self) -> impl Iterator<Item = (&str, usize)> {
        self.actions
            .iter()
            .enumerate()
            .flat_map(|(i, a)| a.option_strings.iter().map(move |s| (s.as_str(), i)))
    }

    fn lookup(&self, s: &str) -> Option<usize> {
        self.option_strings().find(|(o, _)| *o == s).map(|(_, i)| i)
    }

    // ---------------------------------------------------------------------------------------
    // Formatting

    fn metavar(&self, a: &Argument) -> String {
        if let Some(m) = &a.metavar {
            m.clone()
        } else if let Some(c) = &a.choices {
            format!("{{{}}}", c.join(","))
        } else {
            a.dest.clone().unwrap_or_default().to_uppercase()
        }
    }

    fn format_args(&self, a: &Argument) -> String {
        let m = self.metavar(a);
        match a.nargs {
            Nargs::Single => m,
            Nargs::Optional => format!("[{m}]"),
            Nargs::ZeroOrMore => format!("[{m} ...]"),
            Nargs::OneOrMore => format!("{m} [{m} ...]"),
            Nargs::Exactly(n) => vec![m; n].join(" "),
        }
    }

    fn invocation(&self, a: &Argument) -> String {
        if a.nargs == Nargs::Exactly(0) {
            a.option_strings.join(", ")
        } else {
            let args = self.format_args(a);
            a.option_strings
                .iter()
                .map(|o| format!("{o} {args}"))
                .collect::<Vec<_>>()
                .join(", ")
        }
    }

    fn actions_usage(&self) -> String {
        let parts: Vec<String> = self
            .actions
            .iter()
            .filter(|a| !a.suppressed())
            .map(|a| {
                let part = if a.nargs == Nargs::Exactly(0) {
                    a.option_strings[0].clone()
                } else {
                    format!("{} {}", a.option_strings[0], self.format_args(a))
                };
                if a.required {
                    part
                } else {
                    format!("[{part}]")
                }
            })
            .collect();
        let mut text = parts.join(" ");
        // The clean-ups of mutually exclusive groups' separators.
        for open in ["[ ", "( "] {
            text = text.replace(open, &open[..1]);
        }
        for close in [" ]", " )"] {
            text = text.replace(close, &close[1..]);
        }
        text.trim().to_owned()
    }

    /// `re.findall(r'\(.*?\)+(?=\s|$)|\[.*?\]+(?=\s|$)|\S+', text)`.
    fn usage_parts(text: &str) -> Vec<String> {
        let t: Vec<char> = text.chars().collect();
        let mut out = Vec::new();
        let mut i = 0;
        while i < t.len() {
            if t[i].is_whitespace() {
                i += 1;
                continue;
            }
            let mut end = None;
            if let Some(close) = match t[i] {
                '(' => Some(')'),
                '[' => Some(']'),
                _ => None,
            } {
                let mut p = i + 1;
                while p < t.len() && t[p] != '\n' {
                    if t[p] == close {
                        let mut q = p;
                        while q < t.len() && t[q] == close {
                            q += 1;
                        }
                        if q == t.len() || t[q].is_whitespace() {
                            end = Some(q);
                            break;
                        }
                    }
                    p += 1;
                }
            }
            let end = end.unwrap_or_else(|| {
                let mut q = i;
                while q < t.len() && !t[q].is_whitespace() {
                    q += 1;
                }
                q
            });
            out.push(t[i..end].iter().collect());
            i = end;
        }
        out
    }

    fn format_usage_text(&self, width: isize) -> String {
        let prefix = "usage: ";
        let prog = &self.prog;
        let action_usage = self.actions_usage();
        let usage = [prog.as_str(), action_usage.as_str()]
            .iter()
            .filter(|s| !s.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join(" ");
        let text_width = width;
        let clen = |s: &str| s.chars().count() as isize;
        let usage = if clen(prefix) + clen(&usage) > text_width {
            let opt_parts = Self::usage_parts(&action_usage);
            let get_lines = |parts: &[String], indent: &str, prefix: Option<&str>| {
                let mut lines: Vec<String> = Vec::new();
                let mut line: Vec<&str> = Vec::new();
                let indent_len = clen(indent);
                let mut line_len = match prefix {
                    Some(p) => clen(p) - 1,
                    None => indent_len - 1,
                };
                for part in parts {
                    if line_len + 1 + clen(part) > text_width && !line.is_empty() {
                        lines.push(format!("{indent}{}", line.join(" ")));
                        line.clear();
                        line_len = indent_len - 1;
                    }
                    line.push(part);
                    line_len += clen(part) + 1;
                }
                if !line.is_empty() {
                    lines.push(format!("{indent}{}", line.join(" ")));
                }
                if prefix.is_some() && !lines.is_empty() {
                    lines[0] = lines[0].chars().skip(indent_len as usize).collect();
                }
                lines
            };
            let lines = if 4 * (clen(prefix) + clen(prog)) <= 3 * text_width {
                let indent = " ".repeat(prefix.len() + prog.chars().count() + 1);
                if opt_parts.is_empty() {
                    vec![prog.clone()]
                } else {
                    let mut parts = vec![prog.clone()];
                    parts.extend(opt_parts);
                    get_lines(&parts, &indent, Some(prefix))
                }
            } else {
                let indent = " ".repeat(prefix.len());
                let mut lines = get_lines(&opt_parts, &indent, None);
                if lines.len() > 1 {
                    lines = get_lines(&opt_parts, &indent, None);
                }
                let mut all = vec![prog.clone()];
                all.extend(lines);
                all
            };
            lines.join("\n")
        } else {
            usage
        };
        format!("{prefix}{usage}\n\n")
    }

    /// `parser.format_usage()`.
    pub fn format_usage(&self, width: isize) -> String {
        finish(&self.format_usage_text(width))
    }

    fn help_string(&self, a: &Argument) -> Option<String> {
        // `if not action.help`: no help (None or empty) prints the invocation alone.
        let mut help = a.help.clone().flatten().filter(|h| !h.is_empty())?;
        if self.defaults_help
            && !help.contains("%(default)")
            && a.default.is_some()
            && (!a.option_strings.is_empty()
                || matches!(a.nargs, Nargs::Optional | Nargs::ZeroOrMore))
        {
            help.push_str(" (default: %(default)s)");
        }
        // `_expand_help`: `help % params`.
        let default = a.default.as_ref().map(Value::py_str).unwrap_or_default();
        let choices = a.choices.as_ref().map(|c| c.join(", ")).unwrap_or_default();
        let mut out = String::new();
        let mut rest = help.as_str();
        while let Some(p) = rest.find('%') {
            out.push_str(&rest[..p]);
            let tail = &rest[p..];
            if let Some(t) = tail.strip_prefix("%%") {
                out.push('%');
                rest = t;
            } else if let Some(close) = tail.find(")s").filter(|_| tail.starts_with("%(")) {
                let key = &tail[2..close];
                out.push_str(&match key {
                    "default" => default.clone(),
                    "prog" => self.prog.clone(),
                    "dest" => a.dest.clone().unwrap_or_default(),
                    "metavar" => a.metavar.clone().unwrap_or_else(|| "None".into()),
                    "choices" => choices.clone(),
                    "required" => if a.required { "True" } else { "False" }.into(),
                    other => format!("%({other})s"),
                });
                rest = &tail[close + 2..];
            } else {
                out.push('%');
                rest = &tail[1..];
            }
        }
        out.push_str(rest);
        Some(out)
    }

    /// `parser.format_help()`.
    pub fn format_help(&self, width: isize) -> String {
        let indent_increment = 2isize;
        let max_help_position = 24.min((width - 20).max(indent_increment * 2));
        // The longest invocation, at the sections' indentation.
        let action_max_length = self
            .actions
            .iter()
            .filter(|a| !a.suppressed())
            .map(|a| self.invocation(a).chars().count() as isize + indent_increment)
            .max()
            .unwrap_or(0);
        let help_position = (action_max_length + 2).min(max_help_position);
        let help_width = (width - help_position).max(11) as usize;
        let current_indent = indent_increment;
        let action_width = help_position - current_indent - 2;
        let mut out = self.format_usage_text(width);
        for (title, members) in &self.groups {
            let mut items = String::new();
            for &i in members {
                let a = &self.actions[i];
                if a.suppressed() {
                    continue;
                }
                let header = self.invocation(a);
                let ind = " ".repeat(current_indent as usize);
                let help = self.help_string(a);
                let (head, indent_first) = match &help {
                    None => (format!("{ind}{header}\n"), 0),
                    Some(_) if header.chars().count() as isize <= action_width => (
                        format!("{ind}{header:<w$}  ", w = action_width.max(0) as usize),
                        0,
                    ),
                    Some(_) => (format!("{ind}{header}\n"), help_position as usize),
                };
                items.push_str(&head);
                match help.filter(|h| !h.trim().is_empty()) {
                    Some(h) => {
                        let norm = h.split_ascii_whitespace().collect::<Vec<_>>().join(" ");
                        let lines = textwrap::wrap(&norm, help_width);
                        if let Some((first, rest)) = lines.split_first() {
                            items.push_str(&format!("{}{first}\n", " ".repeat(indent_first)));
                            for l in rest {
                                items.push_str(&format!(
                                    "{}{l}\n",
                                    " ".repeat(help_position as usize)
                                ));
                            }
                        }
                    }
                    None if !head.ends_with('\n') => items.push('\n'),
                    None => {}
                }
            }
            if !items.is_empty() {
                out.push_str(&format!("\n{title}:\n{items}\n"));
            }
        }
        finish(&out)
    }

    /// The text `action="version"` prints.
    fn version_text(&self, text: &str, width: isize) -> String {
        let text = text.replace("%(prog)s", &self.prog);
        let w = width.max(11) as usize;
        let norm = text.split_ascii_whitespace().collect::<Vec<_>>().join(" ");
        finish(&format!("{}\n\n", textwrap::fill(&norm, w, "")))
    }

    fn error(&self, message: &str, width: isize) -> Exit {
        Exit {
            status: 2,
            stdout: String::new(),
            stderr: format!(
                "{}{}: error: {message}\n",
                self.format_usage(width),
                self.prog
            ),
        }
    }

    // ---------------------------------------------------------------------------------------
    // Parsing

    fn parse_optional(&self, arg: &str) -> Option<Vec<OptTuple>> {
        if arg.is_empty() || !arg.starts_with('-') {
            return None;
        }
        if let Some(i) = self.lookup(arg) {
            return Some(vec![OptTuple {
                action: Some(i),
                option_string: arg.to_owned(),
                sep: None,
                explicit: None,
            }]);
        }
        if arg.chars().count() == 1 {
            return None;
        }
        if let Some((opt, explicit)) = arg.split_once('=')
            && let Some(i) = self.lookup(opt)
        {
            return Some(vec![OptTuple {
                action: Some(i),
                option_string: opt.to_owned(),
                sep: Some("=".into()),
                explicit: Some(explicit.to_owned()),
            }]);
        }
        let tuples = self.option_tuples(arg);
        if !tuples.is_empty() {
            return Some(tuples);
        }
        if is_negative_number(arg) {
            // No option of this parser looks like a negative number.
            return None;
        }
        if arg.contains(' ') {
            return None;
        }
        Some(vec![OptTuple {
            action: None,
            option_string: arg.to_owned(),
            sep: None,
            explicit: None,
        }])
    }

    fn option_tuples(&self, s: &str) -> Vec<OptTuple> {
        let mut out = Vec::new();
        let (prefix, sep, explicit) = match s.split_once('=') {
            Some((p, e)) => (p, Some("=".to_owned()), Some(e.to_owned())),
            None => (s, None, None),
        };
        if s.starts_with("--") {
            for (o, i) in self.option_strings() {
                if o.starts_with(prefix) {
                    out.push(OptTuple {
                        action: Some(i),
                        option_string: o.to_owned(),
                        sep: sep.clone(),
                        explicit: explicit.clone(),
                    });
                }
            }
        } else {
            let short: String = s.chars().take(2).collect();
            let short_explicit: String = s.chars().skip(2).collect();
            for (o, i) in self.option_strings() {
                if o == short {
                    out.push(OptTuple {
                        action: Some(i),
                        option_string: o.to_owned(),
                        sep: Some(String::new()),
                        explicit: Some(short_explicit.clone()),
                    });
                } else if o.starts_with(prefix) {
                    out.push(OptTuple {
                        action: Some(i),
                        option_string: o.to_owned(),
                        sep: sep.clone(),
                        explicit: explicit.clone(),
                    });
                }
            }
        }
        out
    }

    /// `_match_argument` for an option: the number of strings it takes from `pattern`.
    fn match_argument(&self, i: usize, pattern: &[u8]) -> Result<usize, Stop> {
        let a = &self.actions[i];
        let leading_a = pattern.iter().take_while(|&&c| c == b'A').count();
        let fail = |m: String| Err(Stop::Error(Some(i), m));
        match a.nargs {
            Nargs::Single if leading_a >= 1 => Ok(1),
            Nargs::Single => fail("expected one argument".into()),
            Nargs::Optional => Ok(leading_a.min(1)),
            Nargs::ZeroOrMore => Ok(leading_a),
            Nargs::OneOrMore if leading_a >= 1 => Ok(leading_a),
            Nargs::OneOrMore => fail("expected at least one argument".into()),
            Nargs::Exactly(n) => {
                if pattern.len() >= n && pattern[..n].iter().all(|&c| c != b'-') {
                    Ok(n)
                } else if n == 1 {
                    fail("expected 1 argument".into())
                } else {
                    fail(format!("expected {n} arguments"))
                }
            }
        }
    }

    fn convert(&self, i: usize, s: &str) -> Result<Value, Stop> {
        let a = &self.actions[i];
        let v = match a.ty.convert(s) {
            Conversion::Ok(v) => v,
            Conversion::Invalid => {
                return Err(Stop::Error(
                    Some(i),
                    format!("invalid {} value: {}", a.ty.name(), py_repr_str(s)),
                ));
            }
            Conversion::TypeError(m) => return Err(Stop::Error(Some(i), m)),
            Conversion::ParserError(m) => return Err(Stop::Error(None, m)),
        };
        if let Some(choices) = &a.choices {
            let sv = v.py_str();
            if !choices.contains(&sv) {
                return Err(Stop::Error(
                    Some(i),
                    format!(
                        "invalid choice: {} (choose from {})",
                        py_repr_str(&sv),
                        choices.join(", ")
                    ),
                ));
            }
        }
        Ok(v)
    }

    fn take_action(
        &self,
        i: usize,
        strings: &[String],
        ns: &mut Namespace,
        seen: &mut HashSet<usize>,
        width: isize,
    ) -> Result<(), Stop> {
        seen.insert(i);
        let a = &self.actions[i];
        let value = if strings.is_empty() && a.nargs == Nargs::Optional {
            Value::None
        } else if strings.len() == 1 && matches!(a.nargs, Nargs::Single | Nargs::Optional) {
            self.convert(i, &strings[0])?
        } else {
            Value::List(
                strings
                    .iter()
                    .map(|s| self.convert(i, s))
                    .collect::<Result<_, _>>()?,
            )
        };
        let dest = a.dest.as_deref().unwrap_or("");
        match &a.kind {
            Kind::Store => ns.set(dest, value),
            Kind::StoreTrue => ns.set(dest, Value::Bool(true)),
            Kind::StoreFalse => ns.set(dest, Value::Bool(false)),
            Kind::Help => {
                return Err(Stop::Exit(Exit {
                    status: 0,
                    stdout: self.format_help(width),
                    stderr: String::new(),
                }));
            }
            Kind::Version(t) => {
                return Err(Stop::Exit(Exit {
                    status: 0,
                    stdout: self.version_text(t, width),
                    stderr: String::new(),
                }));
            }
        }
        Ok(())
    }

    /// `parser.parse_args(args)` (without the program name), formatting with `width`
    /// ([`formatter_width`]).
    pub fn parse(&self, args: &[String], width: isize) -> Result<Namespace, Exit> {
        self.parse_given(args, width).map(|(ns, _)| ns)
    }

    /// [`Parser::parse`], and the destinations of the arguments the command line gave (argparse's
    /// `seen_actions`, which `parse_args` does not return; larmorx's addition): a value equal to
    /// its default tells apart from one that was given (a replica's own thread option, which
    /// wins over larmorx's `--n-threads` only when it is given).
    pub fn parse_given(
        &self,
        args: &[String],
        width: isize,
    ) -> Result<(Namespace, Vec<String>), Exit> {
        self.parse_inner(args, width).map_err(|stop| match stop {
            Stop::Exit(e) => e,
            Stop::Error(action, m) => {
                let msg = match action {
                    Some(i) => format!("argument {}: {m}", self.actions[i].name()),
                    None => m,
                };
                self.error(&msg, width)
            }
        })
    }

    fn parse_inner(&self, args: &[String], width: isize) -> Result<(Namespace, Vec<String>), Stop> {
        let mut ns = Namespace::default();
        for a in &self.actions {
            if let (Some(dest), Some(default)) = (&a.dest, &a.default)
                && !ns.has(dest)
            {
                ns.set(dest, default.clone());
            }
        }
        let mut pattern: Vec<u8> = Vec::with_capacity(args.len());
        let mut options: Vec<Option<Vec<OptTuple>>> = Vec::with_capacity(args.len());
        let mut after_dashes = false;
        for a in args {
            if after_dashes {
                pattern.push(b'A');
                options.push(None);
            } else if a == "--" {
                pattern.push(b'-');
                options.push(None);
                after_dashes = true;
            } else {
                match self.parse_optional(a) {
                    None => {
                        pattern.push(b'A');
                        options.push(None);
                    }
                    Some(t) => {
                        pattern.push(b'O');
                        options.push(Some(t));
                    }
                }
            }
        }
        let mut seen = HashSet::new();
        let mut extras: Vec<String> = Vec::new();
        let max_option = options.iter().rposition(Option::is_some);
        let mut start = 0usize;
        if let Some(max_option) = max_option {
            while start <= max_option {
                let next = (start..args.len())
                    .find(|&k| options[k].is_some())
                    .expect("an option at or before max_option");
                if start != next {
                    extras.extend(args[start..next].iter().cloned());
                    start = next;
                }
                start = self.consume_optional(
                    start,
                    args,
                    &pattern,
                    options[start].as_ref().expect("an option"),
                    &mut ns,
                    &mut seen,
                    &mut extras,
                    width,
                )?;
            }
        }
        extras.extend(args[start..].iter().cloned());
        let required: Vec<String> = self
            .actions
            .iter()
            .enumerate()
            .filter(|(i, a)| a.required && !seen.contains(i))
            .map(|(_, a)| a.name())
            .collect();
        if !required.is_empty() {
            return Err(Stop::Error(
                None,
                format!(
                    "the following arguments are required: {}",
                    required.join(", ")
                ),
            ));
        }
        // Defaults given as strings go through the type function (identity for `str`).
        for (i, a) in self.actions.iter().enumerate() {
            if seen.contains(&i) {
                continue;
            }
            if let (Some(dest), Some(Value::Str(d))) = (&a.dest, &a.default)
                && !matches!(a.ty, Type::Str)
                && ns.get(dest) == &Value::Str(d.clone())
            {
                let v = self.convert(i, d)?;
                ns.set(dest, v);
            }
        }
        if !extras.is_empty() {
            return Err(Stop::Error(
                None,
                format!("unrecognized arguments: {}", extras.join(" ")),
            ));
        }
        let given = self
            .actions
            .iter()
            .enumerate()
            .filter(|(i, _)| seen.contains(i))
            .filter_map(|(_, a)| a.dest.clone())
            .collect();
        Ok((ns, given))
    }

    #[allow(clippy::too_many_arguments)]
    fn consume_optional(
        &self,
        start: usize,
        args: &[String],
        pattern: &[u8],
        tuples: &[OptTuple],
        ns: &mut Namespace,
        seen: &mut HashSet<usize>,
        extras: &mut Vec<String>,
        width: isize,
    ) -> Result<usize, Stop> {
        if tuples.len() > 1 {
            let opts: Vec<&str> = tuples.iter().map(|t| t.option_string.as_str()).collect();
            return Err(Stop::Error(
                None,
                format!(
                    "ambiguous option: {} could match {}",
                    args[start],
                    opts.join(", ")
                ),
            ));
        }
        let t = &tuples[0];
        let mut action = t.action;
        let mut option_string = t.option_string.clone();
        let mut sep = t.sep.clone();
        let mut explicit = t.explicit.clone();
        let mut taken: Vec<(usize, Vec<String>)> = Vec::new();
        let stop;
        loop {
            let Some(i) = action else {
                extras.push(args[start].clone());
                return Ok(start + 1);
            };
            if let Some(exp) = explicit.clone() {
                let count = self.match_argument(i, b"A")?;
                let second = option_string.chars().nth(1);
                if count == 0 && second != Some('-') && !exp.is_empty() {
                    if sep.as_deref().is_some_and(|s| !s.is_empty()) || exp.starts_with('-') {
                        return Err(Stop::Error(
                            Some(i),
                            format!("ignored explicit argument {}", py_repr_str(&exp)),
                        ));
                    }
                    taken.push((i, Vec::new()));
                    let first: char = option_string.chars().next().expect("non-empty");
                    let mut chars = exp.chars();
                    let c = chars.next().expect("non-empty");
                    let rest: String = chars.collect();
                    option_string = format!("{first}{c}");
                    if let Some(j) = self.lookup(&option_string) {
                        action = Some(j);
                        if rest.is_empty() {
                            sep = None;
                            explicit = None;
                        } else if let Some(r) = rest.strip_prefix('=') {
                            sep = Some("=".into());
                            explicit = Some(r.to_owned());
                        } else {
                            sep = Some(String::new());
                            explicit = Some(rest);
                        }
                    } else {
                        extras.push(format!("{first}{exp}"));
                        stop = start + 1;
                        break;
                    }
                } else if count == 1 {
                    stop = start + 1;
                    taken.push((i, vec![exp]));
                    break;
                } else {
                    return Err(Stop::Error(
                        Some(i),
                        format!("ignored explicit argument {}", py_repr_str(&exp)),
                    ));
                }
            } else {
                let s = start + 1;
                let count = self.match_argument(i, &pattern[s..])?;
                stop = s + count;
                taken.push((i, args[s..stop].to_vec()));
                break;
            }
        }
        for (i, strings) in taken {
            self.take_action(i, &strings, ns, seen, width)?;
        }
        Ok(stop)
    }
}

/// `HelpFormatter.format_help`'s last step: runs of three or more newlines become two, and the
/// text ends with exactly one newline.
fn finish(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut newlines = 0;
    for c in text.chars() {
        if c == '\n' {
            newlines += 1;
            if newlines <= 2 {
                out.push(c);
            }
        } else {
            newlines = 0;
            out.push(c);
        }
    }
    let t = out.trim_matches('\n');
    if t.is_empty() {
        String::new()
    } else {
        format!("{t}\n")
    }
}

/// `^-\d+$|^-\d*\.\d+$` (ASCII digits).
fn is_negative_number(s: &str) -> bool {
    let Some(rest) = s.strip_prefix('-') else {
        return false;
    };
    let digits = |t: &str| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit());
    if digits(rest) {
        return true;
    }
    match rest.split_once('.') {
        Some((a, b)) => (a.is_empty() || a.bytes().all(|c| c.is_ascii_digit())) && digits(b),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo() -> Parser {
        let mut p = Parser::new("prog", true);
        let g = p.add_group("Group");
        p.add(
            Some(g),
            Argument::store(&["-d"], "data")
                .nargs(Nargs::OneOrMore)
                .metavar("FILE")
                .required()
                .help("Files."),
        );
        p.add(
            Some(g),
            Argument::store(&["--fit"], "fit")
                .choices(&["a", "b"])
                .default(Value::Str("a".into()))
                .help("Fit."),
        );
        p.add(
            Some(g),
            Argument::store_true(&["--overwrite", "-f"], "overwrite").help("Force."),
        );
        p.add(
            Some(g),
            Argument::store(&["--n"], "n")
                .ty(Type::Int)
                .default(Value::Int(1))
                .help("N."),
        );
        p.add(None, Argument::version(&["-v", "--version"], "prog v1"));
        p
    }

    fn args(s: &[&str]) -> Vec<String> {
        s.iter().map(|a| (*a).to_owned()).collect()
    }

    #[test]
    fn parses_and_refuses_as_argparse() {
        let p = demo();
        let ns = p
            .parse(&args(&["-d", "x", "y", "--fi=b", "-f"]), 78)
            .unwrap();
        assert_eq!(
            ns.get("data"),
            &Value::List(vec![Value::Str("x".into()), Value::Str("y".into())])
        );
        assert_eq!(ns.get("fit"), &Value::Str("b".into()));
        assert!(ns.get("overwrite").as_bool());
        let e = p.parse(&args(&["--fit", "c"]), 78).unwrap_err();
        assert_eq!(e.status, 2);
        assert!(
            e.stderr
                .ends_with("prog: error: argument --fit: invalid choice: 'c' (choose from a, b)\n")
        );
        let e = p.parse(&args(&[]), 78).unwrap_err();
        assert!(
            e.stderr
                .ends_with("prog: error: the following arguments are required: -d\n")
        );
        let e = p.parse(&args(&["-d", "x", "--n", "x1"]), 78).unwrap_err();
        assert!(
            e.stderr
                .ends_with("error: argument --n: invalid int value: 'x1'\n")
        );
        let e = p.parse(&args(&["-d", "x", "-fz"]), 78).unwrap_err();
        assert!(e.stderr.ends_with("error: unrecognized arguments: -z\n"));
        let e = p.parse(&args(&["-v"]), 78).unwrap_err();
        assert_eq!((e.status, e.stdout.as_str()), (0, "prog v1\n"));
        assert_eq!(
            p.format_usage(78),
            "usage: prog [-h] -d FILE [FILE ...] [--fit {a,b}] [--overwrite] [--n N] [-v]\n"
        );
    }

    /// The destinations given, apart from their defaults: an option written with `=`, as an
    /// abbreviation or with its default's own value is given; one left out is not.
    #[test]
    fn tells_what_was_given() {
        let p = demo();
        let (ns, given) = p.parse_given(&args(&["-d", "x", "--fi=a"]), 78).unwrap();
        assert_eq!(given, ["data", "fit"]);
        assert_eq!(ns, p.parse(&args(&["-d", "x"]), 78).unwrap());
        let (ns, given) = p.parse_given(&args(&["--n=1", "-d", "x"]), 78).unwrap();
        assert_eq!(
            (ns.get("n"), given.as_slice()),
            (&Value::Int(1), &["data".to_owned(), "n".to_owned()][..])
        );
        let (_, given) = p.parse_given(&args(&["-d", "x", "-f"]), 78).unwrap();
        assert_eq!(given, ["data", "overwrite"]);
        assert_eq!(
            p.parse_given(&args(&["--n", "2"]), 78).unwrap_err(),
            p.parse(&args(&["--n", "2"]), 78).unwrap_err()
        );
    }
}
