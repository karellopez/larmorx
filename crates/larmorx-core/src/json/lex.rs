// SPDX-License-Identifier: Apache-2.0 AND MIT
//! nlohmann/json 3.2.0's lexer and parser (`lexer::scan` and its scanners, `get`/`unget` and the
//! characters read, `get_token_string`; `parser::sax_parse_internal`, `exception_message`) over
//! its DOM builder: the values, and the first error as its exception.

use super::{Error, Value};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Token {
    Uninitialized,
    True,
    False,
    Null,
    String,
    Unsigned,
    Integer,
    Float,
    BeginArray,
    BeginObject,
    EndArray,
    EndObject,
    NameSeparator,
    ValueSeparator,
    ParseError,
    EndOfInput,
    LiteralOrValue,
}

fn token_name(t: Token) -> &'static str {
    match t {
        Token::Uninitialized => "<uninitialized>",
        Token::True => "true literal",
        Token::False => "false literal",
        Token::Null => "null literal",
        Token::String => "string literal",
        Token::Unsigned | Token::Integer | Token::Float => "number literal",
        Token::BeginArray => "'['",
        Token::BeginObject => "'{'",
        Token::EndArray => "']'",
        Token::EndObject => "'}'",
        Token::NameSeparator => "':'",
        Token::ValueSeparator => "','",
        Token::ParseError => "<parse error>",
        Token::EndOfInput => "end of input",
        Token::LiteralOrValue => "'[', '{', or a literal",
    }
}

const EOF: i32 = -1;

struct Lexer<'a> {
    input: &'a [u8],
    pos: usize,
    current: i32,
    next_unget: bool,
    chars_read: usize,
    token_string: Vec<u8>,
    token_buffer: Vec<u8>,
    error_message: &'static str,
    value_integer: i64,
    value_unsigned: u64,
    value_float: f64,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a [u8]) -> Lexer<'a> {
        Lexer {
            input,
            pos: 0,
            current: EOF,
            next_unget: false,
            chars_read: 0,
            token_string: Vec::new(),
            token_buffer: Vec::new(),
            error_message: "",
            value_integer: 0,
            value_unsigned: 0,
            value_float: 0.0,
        }
    }

    fn get(&mut self) -> i32 {
        self.chars_read += 1;
        if self.next_unget {
            self.next_unget = false;
        } else {
            self.current = match self.input.get(self.pos) {
                Some(&b) => {
                    self.pos += 1;
                    b as i32
                }
                None => EOF,
            };
        }
        if self.current != EOF {
            self.token_string.push(self.current as u8);
        }
        self.current
    }

    fn unget(&mut self) {
        self.next_unget = true;
        self.chars_read -= 1;
        if self.current != EOF {
            self.token_string.pop();
        }
    }

    fn add(&mut self, c: i32) {
        self.token_buffer.push(c as u8);
    }

    fn reset(&mut self) {
        self.token_buffer.clear();
        self.token_string.clear();
        self.token_string.push(self.current as u8);
    }

    fn get_codepoint(&mut self) -> i32 {
        let mut cp = 0;
        for factor in [12, 8, 4, 0] {
            let c = self.get();
            let v = match c {
                0x30..=0x39 => c - 0x30,
                0x41..=0x46 => c - 0x37,
                0x61..=0x66 => c - 0x57,
                _ => return -1,
            };
            cp += v << factor;
        }
        cp
    }

    fn next_byte_in_range(&mut self, ranges: &[(i32, i32)]) -> bool {
        self.add(self.current);
        for &(lo, hi) in ranges {
            let c = self.get();
            if lo <= c && c <= hi {
                self.add(c);
            } else {
                self.error_message = "invalid string: ill-formed UTF-8 byte";
                return false;
            }
        }
        true
    }

    fn add_codepoint(&mut self, cp: i32) {
        if cp < 0x80 {
            self.add(cp);
        } else if cp <= 0x7ff {
            self.add(0xc0 | (cp >> 6));
            self.add(0x80 | (cp & 0x3f));
        } else if cp <= 0xffff {
            self.add(0xe0 | (cp >> 12));
            self.add(0x80 | ((cp >> 6) & 0x3f));
            self.add(0x80 | (cp & 0x3f));
        } else {
            self.add(0xf0 | (cp >> 18));
            self.add(0x80 | ((cp >> 12) & 0x3f));
            self.add(0x80 | ((cp >> 6) & 0x3f));
            self.add(0x80 | (cp & 0x3f));
        }
    }

    fn scan_string(&mut self) -> Token {
        self.reset();
        loop {
            let c = self.get();
            match c {
                EOF => {
                    self.error_message = "invalid string: missing closing quote";
                    return Token::ParseError;
                }
                0x22 => return Token::String,
                0x5c => {
                    let e = self.get();
                    match e {
                        0x22 => self.add(0x22),
                        0x5c => self.add(0x5c),
                        0x2f => self.add(0x2f),
                        0x62 => self.add(0x08),
                        0x66 => self.add(0x0c),
                        0x6e => self.add(0x0a),
                        0x72 => self.add(0x0d),
                        0x74 => self.add(0x09),
                        0x75 => {
                            let cp1 = self.get_codepoint();
                            let mut cp = cp1;
                            if cp1 == -1 {
                                self.error_message =
                                    "invalid string: '\\u' must be followed by 4 hex digits";
                                return Token::ParseError;
                            }
                            if (0xd800..=0xdbff).contains(&cp1) {
                                if self.get() == 0x5c && self.get() == 0x75 {
                                    let cp2 = self.get_codepoint();
                                    if cp2 == -1 {
                                        self.error_message = "invalid string: '\\u' must be followed by 4 hex digits";
                                        return Token::ParseError;
                                    }
                                    if (0xdc00..=0xdfff).contains(&cp2) {
                                        cp = (cp1 << 10) + cp2 - 0x35f_dc00;
                                    } else {
                                        self.error_message = "invalid string: surrogate U+DC00..U+DFFF must be followed by U+DC00..U+DFFF";
                                        return Token::ParseError;
                                    }
                                } else {
                                    self.error_message = "invalid string: surrogate U+DC00..U+DFFF must be followed by U+DC00..U+DFFF";
                                    return Token::ParseError;
                                }
                            } else if (0xdc00..=0xdfff).contains(&cp1) {
                                self.error_message = "invalid string: surrogate U+DC00..U+DFFF must follow U+D800..U+DBFF";
                                return Token::ParseError;
                            }
                            self.add_codepoint(cp);
                        }
                        _ => {
                            self.error_message =
                                "invalid string: forbidden character after backslash";
                            return Token::ParseError;
                        }
                    }
                }
                0x00..=0x1f => {
                    self.error_message = "invalid string: control character must be escaped";
                    return Token::ParseError;
                }
                0x20..=0x7f => self.add(c),
                0xc2..=0xdf => {
                    if !self.next_byte_in_range(&[(0x80, 0xbf)]) {
                        return Token::ParseError;
                    }
                }
                0xe0 => {
                    if !self.next_byte_in_range(&[(0xa0, 0xbf), (0x80, 0xbf)]) {
                        return Token::ParseError;
                    }
                }
                0xe1..=0xec | 0xee | 0xef => {
                    if !self.next_byte_in_range(&[(0x80, 0xbf), (0x80, 0xbf)]) {
                        return Token::ParseError;
                    }
                }
                0xed => {
                    if !self.next_byte_in_range(&[(0x80, 0x9f), (0x80, 0xbf)]) {
                        return Token::ParseError;
                    }
                }
                0xf0 => {
                    if !self.next_byte_in_range(&[(0x90, 0xbf), (0x80, 0xbf), (0x80, 0xbf)]) {
                        return Token::ParseError;
                    }
                }
                0xf1..=0xf3 => {
                    if !self.next_byte_in_range(&[(0x80, 0xbf), (0x80, 0xbf), (0x80, 0xbf)]) {
                        return Token::ParseError;
                    }
                }
                0xf4 => {
                    if !self.next_byte_in_range(&[(0x80, 0x8f), (0x80, 0xbf), (0x80, 0xbf)]) {
                        return Token::ParseError;
                    }
                }
                _ => {
                    self.error_message = "invalid string: ill-formed UTF-8 byte";
                    return Token::ParseError;
                }
            }
        }
    }

    fn is_digit(c: i32) -> bool {
        (0x30..=0x39).contains(&c)
    }

    fn scan_number(&mut self) -> Token {
        self.reset();
        let mut ty = Token::Unsigned;
        let c = self.current;
        #[derive(PartialEq)]
        enum S {
            Minus,
            Zero,
            Any1,
            Decimal1,
            Decimal2,
            Exponent,
            Sign,
            Any2,
            Done,
        }
        let mut s = if c == 0x2d {
            self.add(c);
            S::Minus
        } else if c == 0x30 {
            self.add(c);
            S::Zero
        } else {
            self.add(c);
            S::Any1
        };
        while s != S::Done {
            s = match s {
                S::Minus => {
                    ty = Token::Integer;
                    let c = self.get();
                    if c == 0x30 {
                        self.add(c);
                        S::Zero
                    } else if (0x31..=0x39).contains(&c) {
                        self.add(c);
                        S::Any1
                    } else {
                        self.error_message = "invalid number; expected digit after '-'";
                        return Token::ParseError;
                    }
                }
                S::Zero => match self.get() {
                    0x2e => {
                        self.add(0x2e);
                        S::Decimal1
                    }
                    c @ (0x65 | 0x45) => {
                        self.add(c);
                        S::Exponent
                    }
                    _ => S::Done,
                },
                S::Any1 => match self.get() {
                    c if Self::is_digit(c) => {
                        self.add(c);
                        S::Any1
                    }
                    0x2e => {
                        self.add(0x2e);
                        S::Decimal1
                    }
                    c @ (0x65 | 0x45) => {
                        self.add(c);
                        S::Exponent
                    }
                    _ => S::Done,
                },
                S::Decimal1 => {
                    ty = Token::Float;
                    let c = self.get();
                    if Self::is_digit(c) {
                        self.add(c);
                        S::Decimal2
                    } else {
                        self.error_message = "invalid number; expected digit after '.'";
                        return Token::ParseError;
                    }
                }
                S::Decimal2 => match self.get() {
                    c if Self::is_digit(c) => {
                        self.add(c);
                        S::Decimal2
                    }
                    c @ (0x65 | 0x45) => {
                        self.add(c);
                        S::Exponent
                    }
                    _ => S::Done,
                },
                S::Exponent => {
                    ty = Token::Float;
                    let c = self.get();
                    if c == 0x2b || c == 0x2d {
                        self.add(c);
                        S::Sign
                    } else if Self::is_digit(c) {
                        self.add(c);
                        S::Any2
                    } else {
                        self.error_message =
                            "invalid number; expected '+', '-', or digit after exponent";
                        return Token::ParseError;
                    }
                }
                S::Sign => {
                    let c = self.get();
                    if Self::is_digit(c) {
                        self.add(c);
                        S::Any2
                    } else {
                        self.error_message = "invalid number; expected digit after exponent sign";
                        return Token::ParseError;
                    }
                }
                S::Any2 => {
                    let c = self.get();
                    if Self::is_digit(c) {
                        self.add(c);
                        S::Any2
                    } else {
                        S::Done
                    }
                }
                S::Done => S::Done,
            };
        }
        self.unget();
        // the token holds only ASCII digits, '-', '.', 'e', 'E', '+'
        let text = std::str::from_utf8(&self.token_buffer).unwrap_or("0");
        if ty == Token::Unsigned {
            // strtoull: no overflow (ERANGE) and the value fits
            if let Ok(v) = text.parse::<u64>() {
                self.value_unsigned = v;
                return Token::Unsigned;
            }
        } else if ty == Token::Integer
            && let Ok(v) = text.parse::<i64>()
        {
            self.value_integer = v;
            return Token::Integer;
        }
        // strtod: correctly rounded, as glibc's (an overflow gives an infinity)
        self.value_float = text.parse::<f64>().unwrap_or(f64::NAN);
        Token::Float
    }

    fn scan_literal(&mut self, text: &[u8], ty: Token) -> Token {
        for &b in &text[1..] {
            if self.get() != b as i32 {
                self.error_message = "invalid literal";
                return Token::ParseError;
            }
        }
        ty
    }

    fn skip_bom(&mut self) -> bool {
        if self.get() == 0xef {
            self.get() == 0xbb && self.get() == 0xbf
        } else {
            self.unget();
            true
        }
    }

    fn scan(&mut self) -> Token {
        if self.chars_read == 0 && !self.skip_bom() {
            self.error_message = "invalid BOM; must be 0xEF 0xBB 0xBF if given";
            return Token::ParseError;
        }
        loop {
            let c = self.get();
            if !(c == 0x20 || c == 0x09 || c == 0x0a || c == 0x0d) {
                break;
            }
        }
        match self.current {
            0x5b => Token::BeginArray,
            0x5d => Token::EndArray,
            0x7b => Token::BeginObject,
            0x7d => Token::EndObject,
            0x3a => Token::NameSeparator,
            0x2c => Token::ValueSeparator,
            0x74 => self.scan_literal(b"true", Token::True),
            0x66 => self.scan_literal(b"false", Token::False),
            0x6e => self.scan_literal(b"null", Token::Null),
            0x22 => self.scan_string(),
            0x2d | 0x30..=0x39 => self.scan_number(),
            0x00 | EOF => Token::EndOfInput,
            _ => {
                self.error_message = "invalid literal";
                Token::ParseError
            }
        }
    }

    /// `get_token_string`: the token's bytes, controls as `<U+00XX>`.
    fn token_string(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for &c in &self.token_string {
            if c <= 0x1f {
                out.extend_from_slice(format!("<U+{c:04X}>").as_bytes());
            } else {
                out.push(c);
            }
        }
        out
    }
}

struct Parser<'a> {
    lexer: Lexer<'a>,
    last: Token,
}

/// Where a value goes in the tree being built.
enum Frame {
    Array(Vec<Value>),
    Object(BTreeMap<Vec<u8>, Value>, Vec<u8>),
}

impl Parser<'_> {
    fn get_token(&mut self) -> Token {
        self.last = self.lexer.scan();
        self.last
    }

    fn exception_message(&self, expected: Token) -> Vec<u8> {
        let mut m = b"syntax error - ".to_vec();
        if self.last == Token::ParseError {
            m.extend_from_slice(self.lexer.error_message.as_bytes());
            m.extend_from_slice(b"; last read: '");
            m.extend_from_slice(&self.lexer.token_string());
            m.push(b'\'');
        } else {
            m.extend_from_slice(b"unexpected ");
            m.extend_from_slice(token_name(self.last).as_bytes());
        }
        if expected != Token::Uninitialized {
            m.extend_from_slice(b"; expected ");
            m.extend_from_slice(token_name(expected).as_bytes());
        }
        m
    }

    fn error(&self, expected: Token) -> Error {
        Error::parse(
            101,
            self.lexer.chars_read,
            &self.exception_message(expected),
        )
    }

    /// Hands `v` to the frame on top (or makes it the result).
    fn put(stack: &mut [Frame], result: &mut Option<Value>, v: Value) {
        match stack.last_mut() {
            Some(Frame::Array(a)) => a.push(v),
            Some(Frame::Object(o, key)) => {
                o.insert(std::mem::take(key), v);
            }
            None => *result = Some(v),
        }
    }

    /// `sax_parse_internal` with the DOM builder.
    fn parse_value(&mut self) -> Result<Value, Error> {
        let mut stack: Vec<Frame> = Vec::new();
        let mut result: Option<Value> = None;
        let mut skip = false;
        loop {
            if !skip {
                match self.last {
                    Token::BeginObject => {
                        if self.get_token() == Token::EndObject {
                            Self::put(&mut stack, &mut result, Value::Object(BTreeMap::new()));
                        } else {
                            if self.last != Token::String {
                                return Err(self.error(Token::String));
                            }
                            let key = self.lexer.token_buffer.clone();
                            if self.get_token() != Token::NameSeparator {
                                return Err(self.error(Token::NameSeparator));
                            }
                            stack.push(Frame::Object(BTreeMap::new(), key));
                            self.get_token();
                            continue;
                        }
                    }
                    Token::BeginArray => {
                        if self.get_token() == Token::EndArray {
                            Self::put(&mut stack, &mut result, Value::Array(Vec::new()));
                        } else {
                            stack.push(Frame::Array(Vec::new()));
                            continue;
                        }
                    }
                    Token::Float => {
                        let v = self.lexer.value_float;
                        if !v.is_finite() {
                            let mut w = b"number overflow parsing '".to_vec();
                            w.extend_from_slice(&self.lexer.token_string());
                            w.push(b'\'');
                            return Err(Error::with_bytes("out_of_range", 406, &w));
                        }
                        Self::put(&mut stack, &mut result, Value::Float(v));
                    }
                    Token::False => Self::put(&mut stack, &mut result, Value::Boolean(false)),
                    Token::Null => Self::put(&mut stack, &mut result, Value::Null),
                    Token::True => Self::put(&mut stack, &mut result, Value::Boolean(true)),
                    Token::Integer => {
                        let v = self.lexer.value_integer;
                        Self::put(&mut stack, &mut result, Value::Integer(v));
                    }
                    Token::String => {
                        let v = self.lexer.token_buffer.clone();
                        Self::put(&mut stack, &mut result, Value::String(v));
                    }
                    Token::Unsigned => {
                        let v = self.lexer.value_unsigned;
                        Self::put(&mut stack, &mut result, Value::Unsigned(v));
                    }
                    Token::ParseError => return Err(self.error(Token::Uninitialized)),
                    _ => return Err(self.error(Token::LiteralOrValue)),
                }
            } else {
                skip = false;
            }
            let Some(top) = stack.last() else {
                return Ok(result.unwrap_or_default());
            };
            if matches!(top, Frame::Array(_)) {
                if self.get_token() == Token::ValueSeparator {
                    self.get_token();
                    continue;
                }
                if self.last == Token::EndArray {
                    let Some(Frame::Array(a)) = stack.pop() else {
                        unreachable!()
                    };
                    Self::put(&mut stack, &mut result, Value::Array(a));
                    skip = true;
                    continue;
                }
                return Err(self.error(Token::EndArray));
            } else {
                if self.get_token() == Token::ValueSeparator {
                    if self.get_token() != Token::String {
                        return Err(self.error(Token::String));
                    }
                    let key = self.lexer.token_buffer.clone();
                    if self.get_token() != Token::NameSeparator {
                        return Err(self.error(Token::NameSeparator));
                    }
                    if let Some(Frame::Object(_, k)) = stack.last_mut() {
                        *k = key;
                    }
                    self.get_token();
                    continue;
                }
                if self.last == Token::EndObject {
                    let Some(Frame::Object(o, _)) = stack.pop() else {
                        unreachable!()
                    };
                    Self::put(&mut stack, &mut result, Value::Object(o));
                    skip = true;
                    continue;
                }
                return Err(self.error(Token::EndObject));
            }
        }
    }
}

/// `json::parse(text)` (`strict`: only white space may follow the value) or `is >> j`
/// (`!strict`): the value, or the exception the library throws. With `!strict` the second value
/// is how many bytes the reader took from the input (the rest stays in the stream).
pub fn parse(input: &[u8], strict: bool) -> Result<(Value, usize), Error> {
    let mut p = Parser {
        lexer: Lexer::new(input),
        last: Token::Uninitialized,
    };
    p.get_token();
    let v = p.parse_value()?;
    if strict && p.get_token() != Token::EndOfInput {
        return Err(p.error(Token::EndOfInput));
    }
    Ok((v, p.lexer.pos))
}
