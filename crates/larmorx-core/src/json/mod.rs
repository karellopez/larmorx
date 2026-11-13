// SPDX-License-Identifier: Apache-2.0 AND MIT
//! JSON as nlohmann/json 3.2.0 reads and writes it (Copyright (c) 2013-2018 Niels Lohmann, MIT;
//! NOTICE): a pure-Rust reader with its lexer's values (unsigned and signed 64-bit integers, the
//! numbers that overflow them as `strtod` reads them, strings with their escapes and UTF-8
//! checks) and its exceptions' messages (the position counted in characters read, the last token
//! read), the values' accessors as `basic_json` gives them (with their `type_error`s), and its
//! writer exact: the pretty printer's indentation, `dump_escaped`'s escapes and UTF-8 checks,
//! Grisu2's digits and the exponent format of `to_chars`, the integers. Objects keep their keys
//! sorted bytewise (`std::map<std::string, ...>`); a key read twice keeps its last value.
//!
//! - [`parse`]: `json::parse` (strict: only white space may follow) and `operator>>` (not
//!   strict: what follows the value is left);
//! - [`Value::dump`], [`Value::write_stream`]: `dump(indent, indent_char, ensure_ascii)` and
//!   `operator<<` with a stream's width and fill;
//! - [`to_chars`]: a `double` as the writer prints it.
//!
//! [`Dialect`]: the texts a program's copy of the library may change (FreeSurfer 8.2.0's copy
//! words `operator[]`'s `type_error.305` otherwise). D13: FS-99 (the indentation's growth).
//! Checked byte for byte against nlohmann/json 3.2.0 compiled by GCC 8.4.0
//! (`validation/probes/fs-json`).

mod dtoa;
mod lex;
mod tables;
#[cfg(test)]
mod tests;

pub use dtoa::to_chars;
pub use lex::parse;

use crate::defects::{self, Notices};
use std::collections::BTreeMap;

/// A JSON value (`basic_json` with its default types).
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Value {
    #[default]
    Null,
    Boolean(bool),
    /// `number_integer_t` (`int64_t`): what the lexer reads of a negative integer.
    Integer(i64),
    /// `number_unsigned_t` (`uint64_t`): what it reads of a non-negative integer.
    Unsigned(u64),
    /// `number_float_t` (`double`).
    Float(f64),
    /// `std::string`: bytes (the reader checks UTF-8; a value built otherwise may hold any).
    String(Vec<u8>),
    Array(Vec<Value>),
    /// `std::map<std::string, basic_json>`: keys in bytewise order.
    Object(BTreeMap<Vec<u8>, Value>),
}

/// Values are dropped without recursion (a document nested a million levels deep is no risk).
impl Drop for Value {
    fn drop(&mut self) {
        let mut stack: Vec<Value> = Vec::new();
        match self {
            Value::Array(a) => stack.append(a),
            Value::Object(o) => stack.extend(std::mem::take(o).into_values()),
            _ => return,
        }
        while let Some(mut v) = stack.pop() {
            match &mut v {
                Value::Array(a) => stack.append(a),
                Value::Object(o) => stack.extend(std::mem::take(o).into_values()),
                _ => {}
            }
        }
    }
}

/// The nesting from which FS-99 is noted: nlohmann's writer and destructor recurse once per
/// level and exhaust the stack somewhere past it (depending on the stack's size); larmorx's writer
/// and values do not recurse.
pub const DEEP_NESTING: usize = 10_000;

/// One of the library's exceptions: its kind, id and `what()` text (bytes: a parse error quotes
/// the input's last token as read).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub kind: &'static str,
    pub id: u32,
    pub message: Vec<u8>,
}

impl Error {
    /// `exception::name(kind, id) + what`.
    pub fn new(kind: &'static str, id: u32, what: &str) -> Error {
        Error::with_bytes(kind, id, what.as_bytes())
    }
    /// The same, with a text of bytes.
    pub fn with_bytes(kind: &'static str, id: u32, what: &[u8]) -> Error {
        let mut message = format!("[json.exception.{kind}.{id}] ").into_bytes();
        message.extend_from_slice(what);
        Error { kind, id, message }
    }
    /// `parse_error::create(id, byte, what)`.
    pub fn parse(id: u32, byte: usize, what: &[u8]) -> Error {
        let at = if byte != 0 {
            format!(" at {byte}")
        } else {
            String::new()
        };
        let mut message =
            format!("[json.exception.parse_error.{id}] parse error{at}: ").into_bytes();
        message.extend_from_slice(what);
        Error {
            kind: "parse_error",
            id,
            message,
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&String::from_utf8_lossy(&self.message))
    }
}

impl std::error::Error for Error {}

/// The texts a program's copy of nlohmann/json may change.
pub trait Dialect {
    /// `type_error.305`'s text for `operator[]` with a key (`string`) or an index (`numeric`).
    fn operator_index_message(&self, numeric: bool, type_name: &str) -> String {
        let _ = numeric;
        format!("cannot use operator[] with {type_name}")
    }
}

/// nlohmann/json 3.2.0's own texts.
#[derive(Clone, Copy, Debug, Default)]
pub struct Nlohmann;

impl Dialect for Nlohmann {}

/// What the accessors' conversions do with a value out of the target's range: C++'s casts as
/// GCC compiles them on x86-64.
fn f64_to_i64_x86(v: f64) -> i64 {
    // cvttsd2si: truncation, 0x8000000000000000 for NaN and values out of range
    if v.is_nan() || v >= 9_223_372_036_854_775_808.0 || v < -9_223_372_036_854_775_808.0 {
        i64::MIN
    } else {
        v as i64
    }
}

impl Value {
    /// `type_name()`.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Object(_) => "object",
            Value::Array(_) => "array",
            Value::String(_) => "string",
            Value::Boolean(_) => "boolean",
            _ => "number",
        }
    }

    /// `size()`: null 0, an array's or object's count, any other value 1.
    pub fn size(&self) -> usize {
        match self {
            Value::Null => 0,
            Value::Array(a) => a.len(),
            Value::Object(o) => o.len(),
            _ => 1,
        }
    }

    /// The values a range-for visits: an array's elements, an object's values in key order,
    /// nothing for null, the value itself otherwise.
    pub fn iter(&self) -> Box<dyn Iterator<Item = &Value> + '_> {
        match self {
            Value::Null => Box::new(std::iter::empty()),
            Value::Array(a) => Box::new(a.iter()),
            Value::Object(o) => Box::new(o.values()),
            v => Box::new(std::iter::once(v)),
        }
    }

    /// Non-const `operator[](key)`: null becomes an empty object; a missing key is added with a
    /// null value; any other value is `type_error.305`.
    pub fn index_key(&mut self, key: &[u8], dialect: &dyn Dialect) -> Result<&mut Value, Error> {
        if let Value::Null = self {
            *self = Value::Object(BTreeMap::new());
        }
        match self {
            Value::Object(o) => Ok(o.entry(key.to_vec()).or_default()),
            v => Err(Error::new(
                "type_error",
                305,
                &dialect.operator_index_message(false, v.type_name()),
            )),
        }
    }

    /// Non-const `operator[](idx)`: null becomes an empty array; an index past the end grows
    /// the array with nulls; any other value is `type_error.305`.
    pub fn index(&mut self, idx: usize, dialect: &dyn Dialect) -> Result<&mut Value, Error> {
        if let Value::Null = self {
            *self = Value::Array(Vec::new());
        }
        match self {
            Value::Array(a) => {
                if idx >= a.len() {
                    a.resize(idx + 1, Value::Null);
                }
                Ok(&mut a[idx])
            }
            v => Err(Error::new(
                "type_error",
                305,
                &dialect.operator_index_message(true, v.type_name()),
            )),
        }
    }

    fn number_error(&self) -> Error {
        Error::new(
            "type_error",
            302,
            &format!("type must be number, but is {}", self.type_name()),
        )
    }

    /// `get<double>()` (`get_arithmetic_value`: numbers only).
    pub fn get_f64(&self) -> Result<f64, Error> {
        match *self {
            Value::Unsigned(u) => Ok(u as f64),
            Value::Integer(i) => Ok(i as f64),
            Value::Float(f) => Ok(f),
            _ => Err(self.number_error()),
        }
    }

    /// `get<float>()` (the arithmetic `from_json`: numbers and booleans).
    pub fn get_f32(&self) -> Result<f32, Error> {
        match *self {
            Value::Unsigned(u) => Ok(u as f32),
            Value::Integer(i) => Ok(i as f32),
            Value::Float(f) => Ok(f as f32),
            Value::Boolean(b) => Ok(b as u8 as f32),
            _ => Err(self.number_error()),
        }
    }

    /// `get<int64_t>()` (`get_arithmetic_value`: numbers only; a `double` out of range gives
    /// x86-64's indefinite integer).
    pub fn get_i64(&self) -> Result<i64, Error> {
        match *self {
            Value::Unsigned(u) => Ok(u as i64),
            Value::Integer(i) => Ok(i),
            Value::Float(f) => Ok(f64_to_i64_x86(f)),
            _ => Err(self.number_error()),
        }
    }

    /// `get<int>()` (the arithmetic `from_json`: numbers and booleans; casts as x86-64's).
    pub fn get_i32(&self) -> Result<i32, Error> {
        match *self {
            Value::Unsigned(u) => Ok(u as i32),
            Value::Integer(i) => Ok(i as i32),
            Value::Float(f) => {
                // cvttsd2si into 32 bits: 0x80000000 for NaN and values out of range
                if f.is_nan() || f >= 2_147_483_648.0 || f <= -2_147_483_649.0 {
                    Ok(i32::MIN)
                } else {
                    Ok(f as i32)
                }
            }
            Value::Boolean(b) => Ok(b as i32),
            _ => Err(self.number_error()),
        }
    }

    /// `get<std::string>()`.
    pub fn get_string(&self) -> Result<&[u8], Error> {
        match self {
            Value::String(s) => Ok(s),
            v => Err(Error::new(
                "type_error",
                302,
                &format!("type must be string, but is {}", v.type_name()),
            )),
        }
    }

    /// `dump(indent, indent_char, ensure_ascii)`: compact for a negative indent, pretty
    /// otherwise.
    pub fn dump(
        &self,
        indent: i32,
        indent_char: u8,
        ensure_ascii: bool,
        notices: &mut Notices,
    ) -> Result<Vec<u8>, Error> {
        let mut s = Serializer::new(indent_char);
        if indent >= 0 {
            s.dump(self, true, ensure_ascii, indent as usize, 0, notices)?;
        } else {
            s.dump(self, false, ensure_ascii, 0, 0, notices)?;
        }
        Ok(s.out)
    }

    /// `operator<<(std::ostream&, const basic_json&)`: pretty with the stream's width as the
    /// indent (and its fill as the indent character) when the width is positive, compact
    /// otherwise; never ASCII-only (the stream's width is reset to 0 after it). The bytes the
    /// stream received, and the exception that stopped the writing, if any (what was written
    /// before it stays in the stream).
    pub fn write_stream(
        &self,
        width: i64,
        fill: u8,
        notices: &mut Notices,
    ) -> (Vec<u8>, Result<(), Error>) {
        let mut s = Serializer::new(fill);
        let pretty = width > 0;
        let r = s.dump(
            self,
            pretty,
            false,
            if pretty { width as usize } else { 0 },
            0,
            notices,
        );
        (s.out, r)
    }
}

/// The serializer's state: the output, the indent string (512 characters at first).
struct Serializer {
    out: Vec<u8>,
    indent_char: u8,
    indent_string: Vec<u8>,
}

impl Serializer {
    fn new(indent_char: u8) -> Serializer {
        Serializer {
            out: Vec::new(),
            indent_char,
            indent_string: vec![indent_char; 512],
        }
    }

    /// The indent string's growth (FS-99): nlohmann doubles it once, with spaces, when an
    /// indent passes its length; an indent more than twice its length is then read past it.
    fn grow(&mut self, new_indent: usize, notices: &mut Notices) {
        if self.indent_string.len() < new_indent {
            let n = self.indent_string.len() * 2;
            let pad = if notices.reproduce() {
                b' '
            } else {
                self.indent_char
            };
            if self.indent_char != b' ' {
                notices.add(
                    &defects::FS_99,
                    if notices.reproduce() {
                        "an indent past 512 characters padded with spaces"
                    } else {
                        "an indent past 512 characters kept in its character"
                    },
                );
            }
            self.indent_string.resize(n, pad);
            if self.indent_string.len() < new_indent {
                notices.add(
                    &defects::FS_99,
                    format!("an indent of {new_indent} characters, past the indent string"),
                );
                let c = self.indent_char;
                self.indent_string.resize(new_indent, c);
            }
        }
    }

    fn indent(&mut self, n: usize) {
        self.out.extend_from_slice(&self.indent_string[..n]);
    }

    /// `serializer::dump`, without recursion: the tasks of the recursive writer, in its order,
    /// on a stack (the indent string grows when a level begins, as nlohmann's does).
    fn dump(
        &mut self,
        val: &Value,
        pretty: bool,
        ensure_ascii: bool,
        step: usize,
        current: usize,
        notices: &mut Notices,
    ) -> Result<(), Error> {
        enum Task<'v> {
            Value(&'v Value, usize, usize),
            Lit(&'static [u8]),
            Indent(usize),
            Key(&'v [u8]),
        }
        let mut tasks = vec![Task::Value(val, current, 1)];
        while let Some(t) = tasks.pop() {
            match t {
                Task::Lit(b) => self.out.extend_from_slice(b),
                Task::Indent(n) => self.indent(n),
                Task::Key(k) => {
                    self.out.push(b'"');
                    self.dump_escaped(k, ensure_ascii)?;
                    self.out
                        .extend_from_slice(if pretty { b"\": " } else { b"\":" });
                }
                Task::Value(v, cur, depth) => {
                    if depth == DEEP_NESTING {
                        notices.add(
                            &defects::FS_99,
                            format!("a value nested {DEEP_NESTING} levels deep or more"),
                        );
                    }
                    match v {
                        Value::Object(o) if !o.is_empty() => {
                            let n = o.len();
                            let mut list: Vec<Task> = Vec::with_capacity(4 * n + 2);
                            if pretty {
                                self.out.extend_from_slice(b"{\n");
                                let new_indent = cur + step;
                                self.grow(new_indent, notices);
                                for (i, (k, e)) in o.iter().enumerate() {
                                    list.push(Task::Indent(new_indent));
                                    list.push(Task::Key(k));
                                    list.push(Task::Value(e, new_indent, depth + 1));
                                    list.push(Task::Lit(if i + 1 < n { b",\n" } else { b"\n" }));
                                }
                                list.push(Task::Indent(cur));
                                list.push(Task::Lit(b"}"));
                            } else {
                                self.out.push(b'{');
                                for (i, (k, e)) in o.iter().enumerate() {
                                    list.push(Task::Key(k));
                                    list.push(Task::Value(e, cur, depth + 1));
                                    if i + 1 < n {
                                        list.push(Task::Lit(b","));
                                    }
                                }
                                list.push(Task::Lit(b"}"));
                            }
                            tasks.extend(list.into_iter().rev());
                        }
                        Value::Array(a) if !a.is_empty() => {
                            let n = a.len();
                            let mut list: Vec<Task> = Vec::with_capacity(3 * n + 2);
                            if pretty {
                                self.out.extend_from_slice(b"[\n");
                                let new_indent = cur + step;
                                self.grow(new_indent, notices);
                                for (i, e) in a.iter().enumerate() {
                                    list.push(Task::Indent(new_indent));
                                    list.push(Task::Value(e, new_indent, depth + 1));
                                    list.push(Task::Lit(if i + 1 < n { b",\n" } else { b"\n" }));
                                }
                                list.push(Task::Indent(cur));
                                list.push(Task::Lit(b"]"));
                            } else {
                                self.out.push(b'[');
                                for (i, e) in a.iter().enumerate() {
                                    list.push(Task::Value(e, cur, depth + 1));
                                    if i + 1 < n {
                                        list.push(Task::Lit(b","));
                                    }
                                }
                                list.push(Task::Lit(b"]"));
                            }
                            tasks.extend(list.into_iter().rev());
                        }
                        Value::Object(_) => self.out.extend_from_slice(b"{}"),
                        Value::Array(_) => self.out.extend_from_slice(b"[]"),
                        Value::String(s) => {
                            self.out.push(b'"');
                            self.dump_escaped(s, ensure_ascii)?;
                            self.out.push(b'"');
                        }
                        Value::Boolean(b) => {
                            self.out
                                .extend_from_slice(if *b { b"true" } else { b"false" })
                        }
                        Value::Integer(i) => self.out.extend_from_slice(i.to_string().as_bytes()),
                        Value::Unsigned(u) => self.out.extend_from_slice(u.to_string().as_bytes()),
                        Value::Float(f) => {
                            if f.is_finite() {
                                self.out.extend_from_slice(to_chars(*f).as_bytes());
                            } else {
                                self.out.extend_from_slice(b"null");
                            }
                        }
                        Value::Null => self.out.extend_from_slice(b"null"),
                    }
                }
            }
        }
        Ok(())
    }

    /// `dump_escaped`: the string checked as UTF-8 (Hoehrmann's decoder), `"`, `\` and the
    /// controls escaped (`\b \t \n \f \r`, others `\u00XX`), with `ensure_ascii` every code point
    /// past 0x7E as `\uXXXX` (surrogate pairs past 0xFFFF). The escaped bytes go through
    /// nlohmann's 512-byte buffer, written out when fewer than 13 bytes are left after a complete
    /// character and at the end: an invalid string leaves out what the buffer held.
    fn dump_escaped(&mut self, s: &[u8], ensure_ascii: bool) -> Result<(), Error> {
        let mut codepoint = 0u32;
        let mut state = 0u8;
        let mut buf: Vec<u8> = Vec::with_capacity(512);
        for (i, &byte) in s.iter().enumerate() {
            let ty = tables::UTF8D[byte as usize];
            codepoint = if state != 0 {
                (byte as u32 & 0x3f) | (codepoint << 6)
            } else {
                (0xffu32 >> ty) & byte as u32
            };
            state = tables::UTF8D[256 + state as usize * 16 + ty as usize];
            match state {
                0 => {
                    match codepoint {
                        0x08 => buf.extend_from_slice(b"\\b"),
                        0x09 => buf.extend_from_slice(b"\\t"),
                        0x0a => buf.extend_from_slice(b"\\n"),
                        0x0c => buf.extend_from_slice(b"\\f"),
                        0x0d => buf.extend_from_slice(b"\\r"),
                        0x22 => buf.extend_from_slice(b"\\\""),
                        0x5c => buf.extend_from_slice(b"\\\\"),
                        c if c <= 0x1f || (ensure_ascii && c >= 0x7f) => {
                            if c <= 0xffff {
                                buf.extend_from_slice(format!("\\u{:04x}", c as u16).as_bytes());
                            } else {
                                let hi = (0xd7c0 + (c >> 10)) as u16;
                                let lo = (0xdc00 + (c & 0x3ff)) as u16;
                                buf.extend_from_slice(format!("\\u{hi:04x}\\u{lo:04x}").as_bytes());
                            }
                        }
                        _ => buf.push(byte),
                    }
                    if 512 - buf.len() < 13 {
                        self.out.extend_from_slice(&buf);
                        buf.clear();
                    }
                }
                1 => {
                    return Err(Error::new(
                        "type_error",
                        316,
                        &format!("invalid UTF-8 byte at index {i}: 0x{byte:02X}"),
                    ));
                }
                _ => {
                    if !ensure_ascii {
                        buf.push(byte);
                    }
                }
            }
        }
        if state != 0 {
            let last = *s.last().unwrap_or(&0);
            return Err(Error::new(
                "type_error",
                316,
                &format!("incomplete UTF-8 string; last byte: 0x{last:02X}"),
            ));
        }
        self.out.extend_from_slice(&buf);
        Ok(())
    }
}
