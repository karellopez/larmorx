// SPDX-License-Identifier: Apache-2.0
//! A pure-Rust XML parser in expat 2.0.1's place for gifticlib
//! (not a port of expat, but the same events and the same failures), from
//! expat's behaviour (its tokenizers' and processors' rules as `xmltok_impl.c` and `xmlparse.c`
//! define them; MIT, `PROVENANCE.md`), proved by differential tests against expat in
//! `validation/probes/fs-gifti`.
//!
//! What a caller sees as expat's: a push parser fed in pieces ([`Parser::parse`], expat's
//! `XML_Parse` with `XML_CONTEXT_BYTES`), so character data arrive in expat's chunks (a chunk
//! ends at a markup character, at a line break, which is reported alone as `"\n"`, at a `]`, and at
//! the end of the piece fed; a CR or a cut UTF-8 character at a piece's end waits for the next);
//! start tags with their attributes normalized (line breaks and tabs to blanks, references
//! replaced); comments and processing instructions with their line breaks normalized; the XML
//! declaration; the document type's name and external identifier; and the error codes with
//! [`error_string`]'s texts and [`Parser::current_line_number`]'s line, at expat's positions.
//!
//! Not yet expat's (gaps, reported by the differential tests' README): an internal DTD subset is
//! skipped to its `]` (expat parses its declarations: element models, attribute defaults that
//! reach the start tags, entities); documents in UTF-16 or ISO-8859-1 are refused with
//! `unknown encoding` (expat converts them).

// expat's functions report a status, the error kept in the parser: `Result<_, ()>` is that status.
#![allow(clippy::result_unit_err)]

/// expat's `XML_Error` codes this parser reports.
pub type ErrorCode = i32;
pub const ERROR_NONE: ErrorCode = 0;
pub const ERROR_SYNTAX: ErrorCode = 2;
pub const ERROR_NO_ELEMENTS: ErrorCode = 3;
pub const ERROR_INVALID_TOKEN: ErrorCode = 4;
pub const ERROR_UNCLOSED_TOKEN: ErrorCode = 5;
pub const ERROR_PARTIAL_CHAR: ErrorCode = 6;
pub const ERROR_TAG_MISMATCH: ErrorCode = 7;
pub const ERROR_DUPLICATE_ATTRIBUTE: ErrorCode = 8;
pub const ERROR_JUNK_AFTER_DOC_ELEMENT: ErrorCode = 9;
pub const ERROR_UNDEFINED_ENTITY: ErrorCode = 11;
pub const ERROR_BAD_CHAR_REF: ErrorCode = 14;
pub const ERROR_MISPLACED_XML_PI: ErrorCode = 17;
pub const ERROR_UNKNOWN_ENCODING: ErrorCode = 18;
pub const ERROR_INCORRECT_ENCODING: ErrorCode = 19;
pub const ERROR_UNCLOSED_CDATA_SECTION: ErrorCode = 20;
pub const ERROR_XML_DECL: ErrorCode = 30;
pub const ERROR_FINISHED: ErrorCode = 36;

const MESSAGES: [&str; 40] = [
    "out of memory",
    "syntax error",
    "no element found",
    "not well-formed (invalid token)",
    "unclosed token",
    "partial character",
    "mismatched tag",
    "duplicate attribute",
    "junk after document element",
    "illegal parameter entity reference",
    "undefined entity",
    "recursive entity reference",
    "asynchronous entity",
    "reference to invalid character number",
    "reference to binary entity",
    "reference to external entity in attribute",
    "XML or text declaration not at start of entity",
    "unknown encoding",
    "encoding specified in XML declaration is incorrect",
    "unclosed CDATA section",
    "error in processing external entity reference",
    "document is not standalone",
    "unexpected parser state - please send a bug report",
    "entity declared in parameter entity",
    "requested feature requires XML_DTD support in Expat",
    "cannot change setting once parsing has begun",
    "unbound prefix",
    "must not undeclare prefix",
    "incomplete markup in parameter entity",
    "XML declaration not well-formed",
    "text declaration not well-formed",
    "illegal character(s) in public id",
    "parser suspended",
    "parser not suspended",
    "parsing aborted",
    "parsing finished",
    "cannot suspend in external parameter entity",
    "reserved prefix (xml) must not be undeclared or bound to another namespace name",
    "reserved prefix (xmlns) must not be declared or undeclared",
    "prefix must not be bound to one of the reserved namespace names",
];

/// `XML_ErrorString`: `None` for code 0 and unknown codes (C's null pointer).
pub fn error_string(code: ErrorCode) -> Option<&'static str> {
    (1..=40)
        .contains(&code)
        .then(|| MESSAGES[code as usize - 1])
}

/// An element declaration's content model (`XML_Content`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Content {
    pub kind: i32,
    pub quant: i32,
    pub name: Option<Vec<u8>>,
    pub children: Vec<Content>,
}

/// The callbacks gifticlib registers; every one has an empty default.
#[allow(unused_variables)]
pub trait Handler {
    fn start_element(&mut self, name: &[u8], attrs: &[(Vec<u8>, Vec<u8>)]) {}
    fn end_element(&mut self, name: &[u8]) {}
    fn characters(&mut self, data: &[u8]) {}
    fn processing_instruction(&mut self, target: &[u8], data: &[u8]) {}
    fn comment(&mut self, data: &[u8]) {}
    fn start_cdata(&mut self) {}
    fn end_cdata(&mut self) {}
    fn default(&mut self, data: &[u8]) {}
    fn xml_decl(&mut self, version: Option<&[u8]>, encoding: Option<&[u8]>, standalone: i32) {}
    fn start_doctype(
        &mut self,
        name: &[u8],
        sysid: Option<&[u8]>,
        pubid: Option<&[u8]>,
        has_internal_subset: bool,
    ) {
    }
    fn end_doctype(&mut self) {}
    fn element_decl(&mut self, name: &[u8], model: &Content) {}
}

// expat's byte types (asciitab.h, utf8tab.h; ':' a name start character, as in expat's tables
// for a parser without namespaces).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Bt {
    NonXml,
    Malform,
    Lt,
    Amp,
    Rsqb,
    Lead2,
    Lead3,
    Lead4,
    Trail,
    Cr,
    Lf,
    Gt,
    Quot,
    Apos,
    Equals,
    Quest,
    Excl,
    Sol,
    Semi,
    Num,
    Lsqb,
    S,
    NmStrt,
    Hex,
    Digit,
    Name,
    Minus,
    Other,
    Percnt,
    Lpar,
    Rpar,
    Ast,
    Plus,
    Comma,
    Verbar,
}

fn bt(b: u8) -> Bt {
    use Bt::*;
    match b {
        b'\t' | b' ' => S,
        b'\n' => Lf,
        b'\r' => Cr,
        0..=0x1f => NonXml,
        b'!' => Excl,
        b'"' => Quot,
        b'#' => Num,
        b'%' => Percnt,
        b'&' => Amp,
        b'\'' => Apos,
        b'(' => Lpar,
        b')' => Rpar,
        b'*' => Ast,
        b'+' => Plus,
        b',' => Comma,
        b'-' => Minus,
        b'.' => Name,
        b'/' => Sol,
        b'0'..=b'9' => Digit,
        b':' => NmStrt,
        b';' => Semi,
        b'<' => Lt,
        b'=' => Equals,
        b'>' => Gt,
        b'?' => Quest,
        b'a'..=b'f' | b'A'..=b'F' => Hex,
        b'g'..=b'z' | b'G'..=b'Z' | b'_' => NmStrt,
        b'[' => Lsqb,
        b']' => Rsqb,
        b'|' => Verbar,
        0x80..=0xbf => Trail,
        0xc0..=0xdf => Lead2,
        0xe0..=0xef => Lead3,
        0xf0..=0xf4 => Lead4,
        0xf5..=0xfd => NonXml,
        0xfe | 0xff => Malform,
        _ => Other,
    }
}

/// `UTF8_INVALID2/3/4`.
fn utf8_invalid(p: &[u8]) -> bool {
    match p.len() {
        2 => p[0] < 0xC2 || (p[1] & 0x80) == 0 || (p[1] & 0xC0) == 0xC0,
        3 => {
            (p[2] & 0x80) == 0
                || (if p[0] == 0xEF && p[1] == 0xBF {
                    p[2] > 0xBD
                } else {
                    (p[2] & 0xC0) == 0xC0
                })
                || (if p[0] == 0xE0 {
                    p[1] < 0xA0 || (p[1] & 0xC0) == 0xC0
                } else {
                    (p[1] & 0x80) == 0
                        || (if p[0] == 0xED {
                            p[1] > 0x9F
                        } else {
                            (p[1] & 0xC0) == 0xC0
                        })
                })
        }
        _ => {
            (p[3] & 0x80) == 0
                || (p[3] & 0xC0) == 0xC0
                || (p[2] & 0x80) == 0
                || (p[2] & 0xC0) == 0xC0
                || (if p[0] == 0xF0 {
                    p[1] < 0x90 || (p[1] & 0xC0) == 0xC0
                } else {
                    (p[1] & 0x80) == 0
                        || (if p[0] == 0xF4 {
                            p[1] > 0x8F
                        } else {
                            (p[1] & 0xC0) == 0xC0
                        })
                })
        }
    }
}

fn lead_len(b: Bt) -> usize {
    match b {
        Bt::Lead2 => 2,
        Bt::Lead3 => 3,
        Bt::Lead4 => 4,
        _ => 0,
    }
}

/// A UTF-8 character's code point (the bytes already checked).
fn code_point(p: &[u8]) -> u32 {
    match p.len() {
        2 => ((p[0] as u32 & 0x1f) << 6) | (p[1] as u32 & 0x3f),
        3 => ((p[0] as u32 & 0x0f) << 12) | ((p[1] as u32 & 0x3f) << 6) | (p[2] as u32 & 0x3f),
        _ => {
            ((p[0] as u32 & 0x07) << 18)
                | ((p[1] as u32 & 0x3f) << 12)
                | ((p[2] as u32 & 0x3f) << 6)
                | (p[3] as u32 & 0x3f)
        }
    }
}

/// Name start and name characters beyond ASCII (XML 1.0's Appendix B as expat's `nametab.h`
/// encodes it, approximated by Unicode's alphabetic and numeric classes; a gap until the
/// differential tests have it exact).
fn nonascii_name(cp: u32, start: bool) -> bool {
    if cp >= 0x10000 {
        return false; // expat's tables have no character above U+FFFF in names
    }
    let Some(c) = char::from_u32(cp) else {
        return false;
    };
    if start {
        c.is_alphabetic()
    } else {
        c.is_alphanumeric() || cp == 0xB7
    }
}

/// A token of the content or prolog scanners: its kind and where it ends (or, for an invalid
/// one, the bad character's position).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tok {
    None,
    Partial,
    PartialChar,
    Invalid(usize),
    TrailingCr,
    TrailingRsqb,
    DataChars(usize),
    DataNewline(usize),
    StartTag(usize, bool),
    EndTag(usize),
    EntityRef(usize),
    CharRef(usize),
    Comment(usize),
    Pi(usize),
    XmlDecl(usize),
    CdataOpen(usize),
    CdataClose(usize),
    // prolog
    PrologS(usize),
    DeclOpen(usize),
    InstanceStart,
    Name(usize),
    Literal(usize),
    OpenBracket(usize),
    Other(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Proc {
    Prolog,
    Content,
    Cdata,
    Epilog,
    Error,
    Finished,
}

/// The parser (`XML_Parser`).
pub struct Parser {
    buf: Vec<u8>,
    /// Bytes of the document before `buf[0]`.
    base: usize,
    /// The next unparsed byte (absolute).
    pos: usize,
    /// Where the current event starts (expat's `eventPtr`, absolute).
    event: usize,
    lines: u64,
    counted: usize,
    prev_cr: bool,
    proc: Proc,
    tags: Vec<Vec<u8>>,
    error: ErrorCode,
    started: bool,
    seen_doctype: bool,
    seen_decl_slot: bool,
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

impl Parser {
    /// `XML_ParserCreate(NULL)`.
    pub fn new() -> Parser {
        Parser {
            buf: Vec::new(),
            base: 0,
            pos: 0,
            event: 0,
            lines: 0,
            counted: 0,
            prev_cr: false,
            proc: Proc::Prolog,
            tags: Vec::new(),
            error: ERROR_NONE,
            started: false,
            seen_doctype: false,
            seen_decl_slot: false,
        }
    }

    /// `XML_GetErrorCode`.
    pub fn error_code(&self) -> ErrorCode {
        self.error
    }

    /// `XML_GetCurrentLineNumber`: the line (from 1) of the current event's start.
    pub fn current_line_number(&mut self) -> u64 {
        let ev = self.event;
        self.count_to(ev);
        self.lines + 1
    }

    fn count_to(&mut self, abs: usize) {
        while self.counted < abs {
            let i = self.counted - self.base;
            if i >= self.buf.len() {
                break;
            }
            let c = self.buf[i];
            if c == b'\n' {
                if !self.prev_cr {
                    self.lines += 1;
                }
                self.prev_cr = false;
            } else if c == b'\r' {
                self.lines += 1;
                self.prev_cr = true;
            } else {
                self.prev_cr = false;
            }
            self.counted += 1;
        }
    }

    fn at(&self, abs: usize) -> u8 {
        self.buf[abs - self.base]
    }
    fn end(&self) -> usize {
        self.base + self.buf.len()
    }
    fn bytes(&self, a: usize, b: usize) -> &[u8] {
        &self.buf[a - self.base..b - self.base]
    }

    fn fail(&mut self, code: ErrorCode, at: usize) -> Result<(), ()> {
        self.event = at;
        self.error = code;
        self.proc = Proc::Error;
        Err(())
    }

    /// `XML_Parse`: parses `data` after what earlier calls left unparsed; `is_final` ends the
    /// document. `Err` after an error ([`Parser::error_code`]).
    pub fn parse(&mut self, data: &[u8], is_final: bool, h: &mut dyn Handler) -> Result<(), ()> {
        match self.proc {
            Proc::Error => return Err(()),
            Proc::Finished => {
                self.error = ERROR_FINISHED;
                return Err(());
            }
            _ => {}
        }
        // drop what is parsed and counted (expat keeps context bytes; nothing observable)
        let keep_from = self.pos.min(self.counted).min(self.event);
        if keep_from > self.base {
            let k = keep_from - self.base;
            self.buf.drain(..k);
            self.base = keep_from;
        }
        self.buf.extend_from_slice(data);
        if !self.started {
            if self.buf.len() < 4 && !is_final && !data.is_empty() {
                // expat waits for the encoding's first bytes
            }
            if self.buf.len() >= 2
                && (self.buf[0..2] == [0xFE, 0xFF] || self.buf[0..2] == [0xFF, 0xFE])
            {
                return self.fail(ERROR_UNKNOWN_ENCODING, self.base);
            }
            if self.buf.len() >= 3 && self.buf[0..3] == [0xEF, 0xBB, 0xBF] {
                self.pos = self.base + 3;
                self.counted = self.pos;
                self.started = true;
            } else if self.buf.len() >= 3 || is_final {
                self.started = true;
            } else if !is_final {
                return Ok(());
            }
        }
        let r = self.run(is_final, h);
        if r.is_ok() && is_final {
            self.proc = Proc::Finished;
        }
        if r.is_ok() {
            let p = self.pos;
            self.count_to(p);
            self.event = p;
        }
        r
    }

    fn run(&mut self, fin: bool, h: &mut dyn Handler) -> Result<(), ()> {
        loop {
            let r = match self.proc {
                Proc::Prolog => self.prolog(fin, h)?,
                Proc::Content => self.content(fin, h)?,
                Proc::Cdata => self.cdata(fin, h)?,
                Proc::Epilog => self.epilog(fin, h)?,
                Proc::Error | Proc::Finished => return Ok(()),
            };
            if !r {
                return Ok(());
            }
        }
    }

    // ---- tokenizers -----------------------------------------------------------------------

    /// One character at `p`: its length, `Err(tok)` for a cut or invalid one.
    fn char_at(&self, p: usize) -> Result<usize, Tok> {
        let b = bt(self.at(p));
        let n = lead_len(b);
        if n == 0 {
            return Ok(1);
        }
        if self.end() - p < n {
            return Err(Tok::PartialChar);
        }
        if utf8_invalid(self.bytes(p, p + n)) {
            return Err(Tok::Invalid(p));
        }
        Ok(n)
    }

    /// Whether the (valid) character at `p` is a name start or name character.
    fn name_char(&self, p: usize, n: usize, start: bool) -> bool {
        if n == 1 {
            let b = bt(self.at(p));
            return matches!(b, Bt::NmStrt | Bt::Hex)
                || (!start && matches!(b, Bt::Digit | Bt::Name | Bt::Minus));
        }
        nonascii_name(code_point(self.bytes(p, p + n)), start)
    }

    /// Scans a name from `p`: its end, or the token that stops it.
    fn scan_name(&self, mut p: usize) -> Result<usize, Tok> {
        while p < self.end() {
            let b = bt(self.at(p));
            if matches!(b, Bt::NonXml | Bt::Malform | Bt::Trail) {
                return Ok(p);
            }
            let n = self.char_at(p)?;
            if !self.name_char(p, n, false) {
                return Ok(p);
            }
            p += n;
        }
        Ok(p)
    }

    fn content_tok(&self, p0: usize) -> Tok {
        let end = self.end();
        if p0 >= end {
            return Tok::None;
        }
        let mut p = p0;
        match bt(self.at(p)) {
            Bt::Lt => return self.scan_lt(p + 1),
            Bt::Amp => return self.scan_ref(p + 1),
            Bt::Cr => {
                p += 1;
                if p == end {
                    return Tok::TrailingCr;
                }
                if bt(self.at(p)) == Bt::Lf {
                    p += 1;
                }
                return Tok::DataNewline(p);
            }
            Bt::Lf => return Tok::DataNewline(p + 1),
            Bt::Rsqb => {
                p += 1;
                if p == end {
                    return Tok::TrailingRsqb;
                }
                if self.at(p) == b']' {
                    p += 1;
                    if p == end {
                        return Tok::TrailingRsqb;
                    }
                    if self.at(p) == b'>' {
                        return Tok::Invalid(p);
                    }
                    p -= 1;
                }
            }
            Bt::NonXml | Bt::Malform | Bt::Trail => return Tok::Invalid(p),
            b if lead_len(b) > 0 => match self.char_at(p) {
                Ok(n) => p += n,
                Err(t) => return t,
            },
            _ => p += 1,
        }
        while p < end {
            match bt(self.at(p)) {
                b if lead_len(b) > 0 => {
                    let n = lead_len(b);
                    if end - p < n || utf8_invalid(self.bytes(p, p + n)) {
                        return Tok::DataChars(p);
                    }
                    p += n;
                }
                Bt::Rsqb => {
                    if p + 1 != end {
                        if self.at(p + 1) != b']' {
                            p += 1;
                            continue;
                        }
                        if p + 2 != end {
                            if self.at(p + 2) != b'>' {
                                p += 1;
                                continue;
                            }
                            return Tok::Invalid(p + 2);
                        }
                    }
                    return Tok::DataChars(p);
                }
                Bt::Amp | Bt::Lt | Bt::NonXml | Bt::Malform | Bt::Trail | Bt::Cr | Bt::Lf => {
                    return Tok::DataChars(p);
                }
                _ => p += 1,
            }
        }
        Tok::DataChars(p)
    }

    fn scan_lt(&self, p: usize) -> Tok {
        let end = self.end();
        if p >= end {
            return Tok::Partial;
        }
        match bt(self.at(p)) {
            Bt::Excl => {
                let q = p + 1;
                if q >= end {
                    return Tok::Partial;
                }
                match self.at(q) {
                    b'-' => self.scan_comment(q + 1),
                    b'[' => self.scan_cdata_open(q + 1),
                    _ => Tok::Invalid(q),
                }
            }
            Bt::Quest => self.scan_pi(p + 1),
            Bt::Sol => self.scan_end_tag(p + 1),
            _ => match self.char_at(p) {
                Err(t) => t,
                Ok(n) if self.name_char(p, n, true) => self.scan_start_tag(p + n),
                Ok(_) => Tok::Invalid(p),
            },
        }
    }

    fn skip_s(&self, mut p: usize) -> usize {
        while p < self.end() && matches!(bt(self.at(p)), Bt::S | Bt::Cr | Bt::Lf) {
            p += 1;
        }
        p
    }

    fn scan_start_tag(&self, p: usize) -> Tok {
        let end = self.end();
        let mut p = match self.scan_name(p) {
            Ok(q) => q,
            Err(t) => return t,
        };
        loop {
            if p >= end {
                return Tok::Partial;
            }
            match bt(self.at(p)) {
                Bt::S | Bt::Cr | Bt::Lf => {
                    p = self.skip_s(p);
                    if p >= end {
                        return Tok::Partial;
                    }
                    match bt(self.at(p)) {
                        Bt::Gt | Bt::Sol => continue,
                        _ => {}
                    }
                    // an attribute
                    match self.char_at(p) {
                        Err(t) => return t,
                        Ok(n) if self.name_char(p, n, true) => {
                            p = match self.scan_name(p + n) {
                                Ok(q) => q,
                                Err(t) => return t,
                            };
                        }
                        Ok(_) => return Tok::Invalid(p),
                    }
                    p = self.skip_s(p);
                    if p >= end {
                        return Tok::Partial;
                    }
                    if self.at(p) != b'=' {
                        return Tok::Invalid(p);
                    }
                    p = self.skip_s(p + 1);
                    if p >= end {
                        return Tok::Partial;
                    }
                    let q = self.at(p);
                    if q != b'"' && q != b'\'' {
                        return Tok::Invalid(p);
                    }
                    p += 1;
                    loop {
                        if p >= end {
                            return Tok::Partial;
                        }
                        let c = self.at(p);
                        if c == q {
                            p += 1;
                            break;
                        }
                        match bt(c) {
                            Bt::Lt | Bt::NonXml | Bt::Malform | Bt::Trail => {
                                return Tok::Invalid(p);
                            }
                            Bt::Amp => match self.scan_ref(p + 1) {
                                Tok::EntityRef(e) | Tok::CharRef(e) => p = e,
                                t => return t,
                            },
                            _ => match self.char_at(p) {
                                Ok(n) => p += n,
                                Err(t) => return t,
                            },
                        }
                    }
                    if p >= end {
                        return Tok::Partial;
                    }
                    match bt(self.at(p)) {
                        Bt::S | Bt::Cr | Bt::Lf | Bt::Gt | Bt::Sol => {}
                        _ => return Tok::Invalid(p),
                    }
                }
                Bt::Gt => return Tok::StartTag(p + 1, false),
                Bt::Sol => {
                    if p + 1 >= end {
                        return Tok::Partial;
                    }
                    if self.at(p + 1) != b'>' {
                        return Tok::Invalid(p + 1);
                    }
                    return Tok::StartTag(p + 2, true);
                }
                _ => return Tok::Invalid(p),
            }
        }
    }

    fn scan_end_tag(&self, p: usize) -> Tok {
        let end = self.end();
        if p >= end {
            return Tok::Partial;
        }
        match self.char_at(p) {
            Err(t) => return t,
            Ok(n) if self.name_char(p, n, true) => {}
            Ok(_) => return Tok::Invalid(p),
        }
        let mut p = match self.scan_name(p) {
            Ok(q) => q,
            Err(t) => return t,
        };
        p = self.skip_s(p);
        if p >= end {
            return Tok::Partial;
        }
        if self.at(p) == b'>' {
            Tok::EndTag(p + 1)
        } else {
            Tok::Invalid(p)
        }
    }

    fn scan_ref(&self, p: usize) -> Tok {
        let end = self.end();
        if p >= end {
            return Tok::Partial;
        }
        if self.at(p) == b'#' {
            let mut q = p + 1;
            if q >= end {
                return Tok::Partial;
            }
            let hex = self.at(q) == b'x';
            if hex {
                q += 1;
                if q >= end {
                    return Tok::Partial;
                }
            }
            let ok = |c: u8| {
                if hex {
                    c.is_ascii_hexdigit()
                } else {
                    c.is_ascii_digit()
                }
            };
            if !ok(self.at(q)) {
                return Tok::Invalid(q);
            }
            while q < end && ok(self.at(q)) {
                q += 1;
            }
            if q >= end {
                return Tok::Partial;
            }
            return if self.at(q) == b';' {
                Tok::CharRef(q + 1)
            } else {
                Tok::Invalid(q)
            };
        }
        match self.char_at(p) {
            Err(t) => return t,
            Ok(n) if self.name_char(p, n, true) => {}
            Ok(_) => return Tok::Invalid(p),
        }
        let q = match self.scan_name(p) {
            Ok(q) => q,
            Err(t) => return t,
        };
        if q >= end {
            return Tok::Partial;
        }
        if self.at(q) == b';' {
            Tok::EntityRef(q + 1)
        } else {
            Tok::Invalid(q)
        }
    }

    fn scan_comment(&self, p: usize) -> Tok {
        let end = self.end();
        if p >= end {
            return Tok::Partial;
        }
        if self.at(p) != b'-' {
            return Tok::Invalid(p);
        }
        let mut p = p + 1;
        while p < end {
            match bt(self.at(p)) {
                Bt::NonXml | Bt::Malform | Bt::Trail => return Tok::Invalid(p),
                Bt::Minus => {
                    p += 1;
                    if p >= end {
                        return Tok::Partial;
                    }
                    if self.at(p) == b'-' {
                        p += 1;
                        if p >= end {
                            return Tok::Partial;
                        }
                        if self.at(p) != b'>' {
                            return Tok::Invalid(p);
                        }
                        return Tok::Comment(p + 1);
                    }
                }
                _ => match self.char_at(p) {
                    Ok(n) => p += n,
                    Err(t) => return t,
                },
            }
        }
        Tok::Partial
    }

    fn scan_cdata_open(&self, p: usize) -> Tok {
        let want = b"CDATA[";
        for (k, &w) in want.iter().enumerate() {
            if p + k >= self.end() {
                return Tok::Partial;
            }
            if self.at(p + k) != w {
                return Tok::Invalid(p + k);
            }
        }
        Tok::CdataOpen(p + want.len())
    }

    fn scan_pi(&self, p: usize) -> Tok {
        let end = self.end();
        if p >= end {
            return Tok::Partial;
        }
        match self.char_at(p) {
            Err(t) => return t,
            Ok(n) if self.name_char(p, n, true) => {}
            Ok(_) => return Tok::Invalid(p),
        }
        let t = match self.scan_name(p) {
            Ok(q) => q,
            Err(tok) => return tok,
        };
        let target = self.bytes(p, t);
        let is_xml = target == b"xml";
        if !is_xml && target.len() == 3 && target.eq_ignore_ascii_case(b"xml") {
            return Tok::Invalid(t);
        }
        if t >= end {
            return Tok::Partial;
        }
        let mut q = t;
        match bt(self.at(q)) {
            Bt::Quest => {
                if q + 1 >= end {
                    return Tok::Partial;
                }
                if self.at(q + 1) == b'>' {
                    return if is_xml {
                        Tok::XmlDecl(q + 2)
                    } else {
                        Tok::Pi(q + 2)
                    };
                }
                return Tok::Invalid(q + 1);
            }
            Bt::S | Bt::Cr | Bt::Lf => {}
            _ => return Tok::Invalid(q),
        }
        while q < end {
            match bt(self.at(q)) {
                Bt::NonXml | Bt::Malform | Bt::Trail => return Tok::Invalid(q),
                Bt::Quest => {
                    q += 1;
                    if q >= end {
                        return Tok::Partial;
                    }
                    if self.at(q) == b'>' {
                        return if is_xml {
                            Tok::XmlDecl(q + 1)
                        } else {
                            Tok::Pi(q + 1)
                        };
                    }
                }
                _ => match self.char_at(q) {
                    Ok(n) => q += n,
                    Err(t) => return t,
                },
            }
        }
        Tok::Partial
    }

    fn prolog_tok(&self, p: usize) -> Tok {
        let end = self.end();
        if p >= end {
            return Tok::None;
        }
        match bt(self.at(p)) {
            Bt::Quot | Bt::Apos => {
                let q = self.at(p);
                let mut r = p + 1;
                while r < end {
                    let c = self.at(r);
                    if c == q {
                        return Tok::Literal(r + 1);
                    }
                    match bt(c) {
                        Bt::NonXml | Bt::Malform | Bt::Trail => return Tok::Invalid(r),
                        _ => match self.char_at(r) {
                            Ok(n) => r += n,
                            Err(t) => return t,
                        },
                    }
                }
                Tok::Partial
            }
            Bt::Lt => {
                let q = p + 1;
                if q >= end {
                    return Tok::Partial;
                }
                match bt(self.at(q)) {
                    Bt::Excl => {
                        let r = q + 1;
                        if r >= end {
                            return Tok::Partial;
                        }
                        match self.at(r) {
                            b'-' => self.scan_comment(r + 1),
                            b'[' => Tok::Other(r + 1),
                            c if matches!(bt(c), Bt::NmStrt | Bt::Hex) => {
                                let mut e = r;
                                while e < end && matches!(bt(self.at(e)), Bt::NmStrt | Bt::Hex) {
                                    e += 1;
                                }
                                if e >= end {
                                    return Tok::Partial;
                                }
                                match bt(self.at(e)) {
                                    Bt::S | Bt::Cr | Bt::Lf | Bt::Percnt => Tok::DeclOpen(e),
                                    _ => Tok::Invalid(e),
                                }
                            }
                            _ => Tok::Invalid(r),
                        }
                    }
                    Bt::Quest => self.scan_pi(q + 1),
                    Bt::NmStrt | Bt::Hex | Bt::Lead2 | Bt::Lead3 | Bt::Lead4 => Tok::InstanceStart,
                    _ => Tok::Invalid(q),
                }
            }
            Bt::S | Bt::Lf | Bt::Cr => {
                let mut q = p;
                loop {
                    q += 1;
                    if q >= end {
                        break;
                    }
                    match bt(self.at(q)) {
                        Bt::S | Bt::Lf => {}
                        Bt::Cr if q + 1 != end => {}
                        _ => return Tok::PrologS(q),
                    }
                }
                Tok::PrologS(q)
            }
            Bt::Lsqb => Tok::OpenBracket(p + 1),
            Bt::Gt
            | Bt::Comma
            | Bt::Verbar
            | Bt::Lpar
            | Bt::Rsqb
            | Bt::Rpar
            | Bt::Percnt
            | Bt::Num => Tok::Other(p + 1),
            Bt::NmStrt | Bt::Hex | Bt::Digit | Bt::Name | Bt::Minus => match self.scan_name(p) {
                Ok(q) if q < end || q == p + 1 => Tok::Name(q.max(p + 1)),
                Ok(_) => Tok::Partial,
                Err(t) => t,
            },
            b if lead_len(b) > 0 => match self.char_at(p) {
                Err(t) => t,
                Ok(n) if self.name_char(p, n, false) => match self.scan_name(p + n) {
                    Ok(q) => Tok::Name(q),
                    Err(t) => t,
                },
                Ok(_) => Tok::Invalid(p),
            },
            _ => Tok::Invalid(p),
        }
    }

    // ---- processors -----------------------------------------------------------------------

    /// The cut-token rule: wait for more data, or `code` at the end of the document.
    fn partial(&mut self, fin: bool, code: ErrorCode, at: usize) -> Result<bool, ()> {
        if !fin {
            return Ok(false);
        }
        self.fail(code, at).map(|_| false)
    }

    fn report_comment(&mut self, s: usize, e: usize, h: &mut dyn Handler) {
        let text = normalize_lines(self.bytes(s + 4, e - 3));
        h.comment(&text);
    }

    fn report_pi(&mut self, s: usize, e: usize, h: &mut dyn Handler) {
        let body = self.bytes(s + 2, e - 2).to_vec();
        let n = body
            .iter()
            .position(|&c| matches!(bt(c), Bt::S | Bt::Cr | Bt::Lf))
            .unwrap_or(body.len());
        let mut d = n;
        while d < body.len() && matches!(bt(body[d]), Bt::S | Bt::Cr | Bt::Lf) {
            d += 1;
        }
        h.processing_instruction(&body[..n], &normalize_lines(&body[d..]));
    }

    fn prolog(&mut self, fin: bool, h: &mut dyn Handler) -> Result<bool, ()> {
        loop {
            let s = self.pos;
            self.event = s;
            let tok = self.prolog_tok(s);
            match tok {
                Tok::None => {
                    return if fin {
                        self.fail(ERROR_NO_ELEMENTS, s).map(|_| false)
                    } else {
                        Ok(false)
                    };
                }
                Tok::Partial => return self.partial(fin, ERROR_UNCLOSED_TOKEN, s),
                Tok::PartialChar => return self.partial(fin, ERROR_PARTIAL_CHAR, s),
                Tok::Invalid(at) => return self.fail(ERROR_INVALID_TOKEN, at).map(|_| false),
                Tok::PrologS(e) => {
                    if e == self.end() && !fin && self.at(e - 1) == b'\r' {
                        return Ok(false);
                    }
                    let d = self.bytes(s, e).to_vec();
                    h.default(&d);
                    self.pos = e;
                }
                Tok::Comment(e) => {
                    self.report_comment(s, e, h);
                    self.pos = e;
                }
                Tok::Pi(e) => {
                    self.report_pi(s, e, h);
                    self.pos = e;
                }
                Tok::XmlDecl(e) => {
                    if s != 0 || self.seen_decl_slot {
                        return self.fail(ERROR_MISPLACED_XML_PI, s).map(|_| false);
                    }
                    self.xml_decl(s, e, h)?;
                    self.pos = e;
                }
                Tok::DeclOpen(e) if !self.seen_doctype && self.bytes(s + 2, e) == b"DOCTYPE" => {
                    match self.doctype(e, fin, h)? {
                        Some(end) => self.pos = end,
                        None => return Ok(false),
                    }
                    self.seen_doctype = true;
                }
                Tok::InstanceStart => {
                    self.proc = Proc::Content;
                    return Ok(true);
                }
                _ => return self.fail(ERROR_SYNTAX, s).map(|_| false),
            }
            self.seen_decl_slot = true;
        }
    }

    /// The XML declaration (`XmlParseXmlDecl`): version, encoding, standalone.
    fn xml_decl(&mut self, s: usize, e: usize, h: &mut dyn Handler) -> Result<(), ()> {
        let body = self.bytes(s + 5, e - 2).to_vec();
        let mut i = 0;
        let mut items: Vec<(Vec<u8>, Vec<u8>, usize)> = Vec::new();
        loop {
            let ws = i;
            while i < body.len() && matches!(bt(body[i]), Bt::S | Bt::Cr | Bt::Lf) {
                i += 1;
            }
            if i >= body.len() {
                break;
            }
            if i == ws {
                return self.fail(ERROR_XML_DECL, s + 5 + i).map(|_| ());
            }
            let n0 = i;
            while i < body.len()
                && matches!(
                    bt(body[i]),
                    Bt::NmStrt | Bt::Hex | Bt::Digit | Bt::Name | Bt::Minus
                )
            {
                i += 1;
            }
            let name = body[n0..i].to_vec();
            while i < body.len() && matches!(bt(body[i]), Bt::S | Bt::Cr | Bt::Lf) {
                i += 1;
            }
            if i >= body.len() || body[i] != b'=' || name.is_empty() {
                return self.fail(ERROR_XML_DECL, s + 5 + n0).map(|_| ());
            }
            i += 1;
            while i < body.len() && matches!(bt(body[i]), Bt::S | Bt::Cr | Bt::Lf) {
                i += 1;
            }
            if i >= body.len() || (body[i] != b'"' && body[i] != b'\'') {
                return self.fail(ERROR_XML_DECL, s + 5 + i).map(|_| ());
            }
            let q = body[i];
            let v0 = i + 1;
            let Some(len) = body[v0..].iter().position(|&c| c == q) else {
                return self.fail(ERROR_XML_DECL, s + 5 + i).map(|_| ());
            };
            items.push((name, body[v0..v0 + len].to_vec(), s + 5 + v0));
            i = v0 + len + 1;
        }
        let mut k = 0;
        let version;
        let mut encoding = None;
        let mut standalone = -1;
        if k < items.len() && items[k].0 == b"version" {
            version = Some(items[k].1.clone());
            k += 1;
        } else {
            let at = items.first().map_or(s, |x| x.2);
            return self.fail(ERROR_XML_DECL, at).map(|_| ());
        }
        if k < items.len() && items[k].0 == b"encoding" {
            let v = &items[k].1;
            let ok = !v.is_empty()
                && v[0].is_ascii_alphabetic()
                && v.iter()
                    .all(|&c| c.is_ascii_alphanumeric() || b"._-".contains(&c));
            if !ok {
                return self.fail(ERROR_XML_DECL, items[k].2).map(|_| ());
            }
            encoding = Some(v.clone());
            k += 1;
        }
        if k < items.len() && items[k].0 == b"standalone" {
            standalone = match items[k].1.as_slice() {
                b"yes" => 1,
                b"no" => 0,
                _ => return self.fail(ERROR_XML_DECL, items[k].2).map(|_| ()),
            };
            k += 1;
        }
        if k < items.len() {
            return self.fail(ERROR_XML_DECL, items[k].2).map(|_| ());
        }
        if let Some(v) = version.as_ref()
            && !v
                .iter()
                .all(|&c| c.is_ascii_alphanumeric() || b"._:-".contains(&c))
        {
            return self.fail(ERROR_XML_DECL, s).map(|_| ());
        }
        h.xml_decl(version.as_deref(), encoding.as_deref(), standalone);
        if let Some(enc) = &encoding {
            let e = enc.to_ascii_uppercase();
            match e.as_slice() {
                b"UTF-8" | b"US-ASCII" => {}
                b"UTF-16" | b"UTF-16BE" | b"UTF-16LE" => {
                    let at = items.iter().find(|x| x.0 == b"encoding").map_or(s, |x| x.2);
                    return self.fail(ERROR_INCORRECT_ENCODING, at).map(|_| ());
                }
                _ => {
                    let at = items.iter().find(|x| x.0 == b"encoding").map_or(s, |x| x.2);
                    return self.fail(ERROR_UNKNOWN_ENCODING, at).map(|_| ());
                }
            }
        }
        Ok(())
    }

    /// `<!DOCTYPE name [external id] [internal subset]>` from after the keyword: its end, or
    /// `None` to wait for more data.
    fn doctype(&mut self, p: usize, fin: bool, h: &mut dyn Handler) -> Result<Option<usize>, ()> {
        let mut toks = Vec::new();
        let mut q = p;
        let mut subset = false;
        loop {
            let t = self.prolog_tok(q);
            match t {
                Tok::None | Tok::Partial => {
                    return if fin {
                        self.fail(ERROR_UNCLOSED_TOKEN, self.pos).map(|_| None)
                    } else {
                        Ok(None)
                    };
                }
                Tok::PartialChar => {
                    return if fin {
                        self.fail(ERROR_PARTIAL_CHAR, q).map(|_| None)
                    } else {
                        Ok(None)
                    };
                }
                Tok::Invalid(at) => return self.fail(ERROR_INVALID_TOKEN, at).map(|_| None),
                Tok::PrologS(e) => q = e,
                Tok::Name(e) | Tok::Literal(e) => {
                    toks.push((t, q, e));
                    q = e;
                }
                Tok::OpenBracket(e) => {
                    subset = true;
                    // skip the internal subset to its `]` (a gap: its declarations are not read)
                    let mut r = e;
                    let mut quote = 0u8;
                    loop {
                        if r >= self.end() {
                            return if fin {
                                self.fail(ERROR_UNCLOSED_TOKEN, self.pos).map(|_| None)
                            } else {
                                Ok(None)
                            };
                        }
                        let c = self.at(r);
                        if quote != 0 {
                            if c == quote {
                                quote = 0;
                            }
                        } else if c == b'"' || c == b'\'' {
                            quote = c;
                        } else if c == b']' {
                            break;
                        }
                        r += 1;
                    }
                    q = r + 1;
                    q = self.skip_s(q);
                    if q >= self.end() {
                        return if fin {
                            self.fail(ERROR_UNCLOSED_TOKEN, self.pos).map(|_| None)
                        } else {
                            Ok(None)
                        };
                    }
                    if self.at(q) != b'>' {
                        return self.fail(ERROR_SYNTAX, q).map(|_| None);
                    }
                    q += 1;
                    break;
                }
                Tok::Other(e) if self.at(q) == b'>' => {
                    q = e;
                    break;
                }
                _ => return self.fail(ERROR_SYNTAX, q).map(|_| None),
            }
        }
        let get = |k: usize| -> Option<Vec<u8>> {
            toks.get(k).map(|&(_, a, b)| self.bytes(a, b).to_vec())
        };
        let lit = |k: usize| -> Option<Vec<u8>> {
            toks.get(k).and_then(|&(t, a, b)| {
                matches!(t, Tok::Literal(_)).then(|| self.bytes(a + 1, b - 1).to_vec())
            })
        };
        let Some(name) = get(0).filter(|_| matches!(toks[0].0, Tok::Name(_))) else {
            let at = toks.first().map_or(q, |x| x.1);
            return self.fail(ERROR_SYNTAX, at).map(|_| None);
        };
        let (mut sysid, mut pubid) = (None, None);
        match get(1).as_deref() {
            None => {}
            Some(b"SYSTEM") if toks.len() == 3 && lit(2).is_some() => sysid = lit(2),
            Some(b"PUBLIC") if toks.len() == 4 && lit(2).is_some() && lit(3).is_some() => {
                pubid = lit(2);
                sysid = lit(3);
            }
            _ => {
                let at = toks.get(1).map_or(q, |x| x.1);
                return self.fail(ERROR_SYNTAX, at).map(|_| None);
            }
        }
        let pubid = pubid.map(|v| normalize_public_id(&v));
        h.start_doctype(&name, sysid.as_deref(), pubid.as_deref(), subset);
        h.end_doctype();
        Ok(Some(q))
    }

    fn content(&mut self, fin: bool, h: &mut dyn Handler) -> Result<bool, ()> {
        loop {
            let s = self.pos;
            self.event = s;
            let tok = self.content_tok(s);
            match tok {
                Tok::None => {
                    return if fin {
                        self.fail(ERROR_NO_ELEMENTS, s).map(|_| false)
                    } else {
                        Ok(false)
                    };
                }
                Tok::TrailingCr => {
                    if !fin {
                        return Ok(false);
                    }
                    h.characters(b"\n");
                    return self.fail(ERROR_NO_ELEMENTS, s).map(|_| false);
                }
                Tok::TrailingRsqb => {
                    if !fin {
                        return Ok(false);
                    }
                    let d = self.bytes(s, self.end()).to_vec();
                    h.characters(&d);
                    return self.fail(ERROR_NO_ELEMENTS, s).map(|_| false);
                }
                Tok::Partial => return self.partial(fin, ERROR_UNCLOSED_TOKEN, s),
                Tok::PartialChar => return self.partial(fin, ERROR_PARTIAL_CHAR, s),
                Tok::Invalid(at) => return self.fail(ERROR_INVALID_TOKEN, at).map(|_| false),
                Tok::DataChars(e) => {
                    let d = self.bytes(s, e).to_vec();
                    h.characters(&d);
                    self.pos = e;
                }
                Tok::DataNewline(e) => {
                    h.characters(b"\n");
                    self.pos = e;
                }
                Tok::CharRef(e) => {
                    let Some(c) = char_ref(self.bytes(s, e)) else {
                        return self.fail(ERROR_BAD_CHAR_REF, s).map(|_| false);
                    };
                    let mut b = [0u8; 4];
                    h.characters(c.encode_utf8(&mut b).as_bytes());
                    self.pos = e;
                }
                Tok::EntityRef(e) => {
                    let name = self.bytes(s + 1, e - 1).to_vec();
                    match predefined(&name) {
                        Some(c) => h.characters(&[c]),
                        None => return self.fail(ERROR_UNDEFINED_ENTITY, s).map(|_| false),
                    }
                    self.pos = e;
                }
                Tok::StartTag(e, empty) => {
                    let (name, attrs) = match self.start_tag(s, e) {
                        Ok(x) => x,
                        Err((code, at)) => return self.fail(code, at).map(|_| false),
                    };
                    h.start_element(&name, &attrs);
                    self.pos = e;
                    if empty {
                        h.end_element(&name);
                        if self.tags.is_empty() {
                            self.proc = Proc::Epilog;
                            return Ok(true);
                        }
                    } else {
                        self.tags.push(name);
                    }
                }
                Tok::EndTag(e) => {
                    let n_end = (s + 2..e)
                        .find(|&k| matches!(bt(self.at(k)), Bt::S | Bt::Cr | Bt::Lf | Bt::Gt))
                        .unwrap_or(e - 1);
                    let name = self.bytes(s + 2, n_end).to_vec();
                    if self.tags.last() != Some(&name) {
                        return self.fail(ERROR_TAG_MISMATCH, s).map(|_| false);
                    }
                    self.tags.pop();
                    h.end_element(&name);
                    self.pos = e;
                    if self.tags.is_empty() {
                        self.proc = Proc::Epilog;
                        return Ok(true);
                    }
                }
                Tok::Comment(e) => {
                    self.report_comment(s, e, h);
                    self.pos = e;
                }
                Tok::Pi(e) => {
                    self.report_pi(s, e, h);
                    self.pos = e;
                }
                Tok::XmlDecl(_) => return self.fail(ERROR_MISPLACED_XML_PI, s).map(|_| false),
                Tok::CdataOpen(e) => {
                    h.start_cdata();
                    self.pos = e;
                    self.proc = Proc::Cdata;
                    return Ok(true);
                }
                _ => return self.fail(ERROR_SYNTAX, s).map(|_| false),
            }
        }
    }

    /// A start tag's name and attributes (values normalized as expat's `storeAtts` does).
    #[allow(clippy::type_complexity)]
    fn start_tag(
        &self,
        s: usize,
        e: usize,
    ) -> Result<(Vec<u8>, Vec<(Vec<u8>, Vec<u8>)>), (ErrorCode, usize)> {
        let mut p = s + 1;
        let n0 = p;
        while !matches!(bt(self.at(p)), Bt::S | Bt::Cr | Bt::Lf | Bt::Gt | Bt::Sol) {
            p += 1;
        }
        let name = self.bytes(n0, p).to_vec();
        let mut attrs: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        loop {
            p = self.skip_s(p);
            if p >= e || matches!(bt(self.at(p)), Bt::Gt | Bt::Sol) {
                break;
            }
            let a0 = p;
            while !matches!(bt(self.at(p)), Bt::S | Bt::Cr | Bt::Lf | Bt::Equals) {
                p += 1;
            }
            let an = self.bytes(a0, p).to_vec();
            p = self.skip_s(p) + 1;
            p = self.skip_s(p);
            let q = self.at(p);
            let v0 = p + 1;
            let mut v1 = v0;
            while self.at(v1) != q {
                v1 += 1;
            }
            if attrs.iter().any(|(n, _)| *n == an) {
                return Err((ERROR_DUPLICATE_ATTRIBUTE, a0));
            }
            let mut val = Vec::new();
            let raw = self.bytes(v0, v1);
            let mut k = 0;
            while k < raw.len() {
                match raw[k] {
                    b'\r' => {
                        val.push(b' ');
                        if k + 1 < raw.len() && raw[k + 1] == b'\n' {
                            k += 1;
                        }
                    }
                    b'\n' | b'\t' => val.push(b' '),
                    b'&' => {
                        let semi = k + raw[k..].iter().position(|&c| c == b';').expect("a ;");
                        let r = &raw[k..=semi];
                        if r[1] == b'#' {
                            let Some(c) = char_ref(r) else {
                                return Err((ERROR_BAD_CHAR_REF, v0 + k));
                            };
                            let mut b = [0u8; 4];
                            val.extend_from_slice(c.encode_utf8(&mut b).as_bytes());
                        } else {
                            match predefined(&r[1..r.len() - 1]) {
                                Some(c) => val.push(c),
                                None => return Err((ERROR_UNDEFINED_ENTITY, v0 + k)),
                            }
                        }
                        k = semi;
                    }
                    c => val.push(c),
                }
                k += 1;
            }
            attrs.push((an, val));
            p = v1 + 1;
        }
        Ok((name, attrs))
    }

    fn cdata(&mut self, fin: bool, h: &mut dyn Handler) -> Result<bool, ()> {
        let end = self.end();
        loop {
            let s = self.pos;
            self.event = s;
            if s >= end {
                return if fin {
                    self.fail(ERROR_UNCLOSED_CDATA_SECTION, s).map(|_| false)
                } else {
                    Ok(false)
                };
            }
            let mut p = s;
            let tok = match bt(self.at(p)) {
                Bt::Rsqb => {
                    p += 1;
                    if p == end {
                        Tok::Partial
                    } else if self.at(p) != b']' {
                        self.cdata_run(p)
                    } else if p + 1 == end {
                        Tok::Partial
                    } else if self.at(p + 1) == b'>' {
                        Tok::CdataClose(p + 2)
                    } else {
                        self.cdata_run(p)
                    }
                }
                Bt::Cr => {
                    p += 1;
                    if p == end {
                        Tok::Partial
                    } else {
                        if bt(self.at(p)) == Bt::Lf {
                            p += 1;
                        }
                        Tok::DataNewline(p)
                    }
                }
                Bt::Lf => Tok::DataNewline(p + 1),
                Bt::NonXml | Bt::Malform | Bt::Trail => Tok::Invalid(p),
                b if lead_len(b) > 0 => match self.char_at(p) {
                    Ok(n) => self.cdata_run(p + n),
                    Err(t) => t,
                },
                _ => self.cdata_run(p + 1),
            };
            match tok {
                Tok::Partial => return self.partial(fin, ERROR_UNCLOSED_CDATA_SECTION, s),
                Tok::PartialChar => return self.partial(fin, ERROR_PARTIAL_CHAR, s),
                Tok::Invalid(at) => return self.fail(ERROR_INVALID_TOKEN, at).map(|_| false),
                Tok::DataChars(e) => {
                    let d = self.bytes(s, e).to_vec();
                    h.characters(&d);
                    self.pos = e;
                }
                Tok::DataNewline(e) => {
                    h.characters(b"\n");
                    self.pos = e;
                }
                Tok::CdataClose(e) => {
                    h.end_cdata();
                    self.pos = e;
                    self.proc = Proc::Content;
                    return Ok(true);
                }
                _ => unreachable!(),
            }
        }
    }

    fn cdata_run(&self, mut p: usize) -> Tok {
        let end = self.end();
        while p < end {
            match bt(self.at(p)) {
                b if lead_len(b) > 0 => {
                    let n = lead_len(b);
                    if end - p < n || utf8_invalid(self.bytes(p, p + n)) {
                        return Tok::DataChars(p);
                    }
                    p += n;
                }
                Bt::NonXml | Bt::Malform | Bt::Trail | Bt::Cr | Bt::Lf | Bt::Rsqb => {
                    return Tok::DataChars(p);
                }
                _ => p += 1,
            }
        }
        Tok::DataChars(p)
    }

    fn epilog(&mut self, fin: bool, h: &mut dyn Handler) -> Result<bool, ()> {
        loop {
            let s = self.pos;
            self.event = s;
            match self.prolog_tok(s) {
                Tok::None => return Ok(false),
                Tok::PrologS(e) => {
                    if e == self.end() && !fin && self.at(e - 1) == b'\r' {
                        return Ok(false);
                    }
                    let d = self.bytes(s, e).to_vec();
                    h.default(&d);
                    self.pos = e;
                }
                Tok::Comment(e) => {
                    self.report_comment(s, e, h);
                    self.pos = e;
                }
                Tok::Pi(e) => {
                    self.report_pi(s, e, h);
                    self.pos = e;
                }
                Tok::Partial => return self.partial(fin, ERROR_UNCLOSED_TOKEN, s),
                Tok::PartialChar => return self.partial(fin, ERROR_PARTIAL_CHAR, s),
                Tok::Invalid(at) => return self.fail(ERROR_INVALID_TOKEN, at).map(|_| false),
                _ => return self.fail(ERROR_JUNK_AFTER_DOC_ELEMENT, s).map(|_| false),
            }
        }
    }
}

/// `normalizeLines`: CR LF and CR become LF.
fn normalize_lines(s: &[u8]) -> Vec<u8> {
    let mut o = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if s[i] == b'\r' {
            o.push(b'\n');
            if i + 1 < s.len() && s[i + 1] == b'\n' {
                i += 1;
            }
        } else {
            o.push(s[i]);
        }
        i += 1;
    }
    o
}

/// `normalizePublicId`: runs of blanks to one, none at the ends.
fn normalize_public_id(s: &[u8]) -> Vec<u8> {
    let mut o: Vec<u8> = Vec::new();
    for &c in s {
        if matches!(c, b' ' | b'\r' | b'\n') {
            if !o.is_empty() && o.last() != Some(&b' ') {
                o.push(b' ');
            }
        } else {
            o.push(c);
        }
    }
    if o.last() == Some(&b' ') {
        o.pop();
    }
    o
}

/// The predefined entities.
fn predefined(name: &[u8]) -> Option<u8> {
    match name {
        b"lt" => Some(b'<'),
        b"gt" => Some(b'>'),
        b"amp" => Some(b'&'),
        b"quot" => Some(b'"'),
        b"apos" => Some(b'\''),
        _ => None,
    }
}

/// `XmlCharRefNumber` with `checkCharRefNumber`: the character of `&#...;`, or `None`.
fn char_ref(r: &[u8]) -> Option<char> {
    let body = &r[2..r.len() - 1];
    let (digits, radix) = if body.first() == Some(&b'x') {
        (&body[1..], 16)
    } else {
        (body, 10)
    };
    let mut v: u32 = 0;
    for &c in digits {
        v = v * radix + (c as char).to_digit(radix)?;
        if v >= 0x110000 {
            return None;
        }
    }
    match v {
        0xD800..=0xDFFF | 0xFFFE | 0xFFFF => None,
        0x09 | 0x0A | 0x0D => char::from_u32(v),
        0..=0x1F => None,
        _ => char::from_u32(v),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Log(Vec<String>);
    impl Handler for Log {
        fn start_element(&mut self, n: &[u8], a: &[(Vec<u8>, Vec<u8>)]) {
            let a: Vec<String> = a
                .iter()
                .map(|(n, v)| format!("{}={}", s(n), s(v)))
                .collect();
            self.0.push(format!("<{} {}>", s(n), a.join(",")));
        }
        fn end_element(&mut self, n: &[u8]) {
            self.0.push(format!("</{}>", s(n)));
        }
        fn characters(&mut self, d: &[u8]) {
            self.0.push(format!("c[{}]", s(d)));
        }
        fn comment(&mut self, d: &[u8]) {
            self.0.push(format!("!{}", s(d)));
        }
        fn start_cdata(&mut self) {
            self.0.push("[[".into());
        }
        fn end_cdata(&mut self) {
            self.0.push("]]".into());
        }
    }
    fn s(b: &[u8]) -> String {
        String::from_utf8_lossy(b).into_owned()
    }
    fn run(pieces: &[&[u8]]) -> (Vec<String>, ErrorCode, u64) {
        let mut p = Parser::new();
        let mut l = <Log as Default>::default();
        for (k, d) in pieces.iter().enumerate() {
            if p.parse(d, k + 1 == pieces.len(), &mut l).is_err() {
                let line = p.current_line_number();
                return (l.0, p.error_code(), line);
            }
        }
        (l.0, 0, 0)
    }

    #[test]
    fn chunks_as_expat() {
        let (ev, e, _) = run(&[
            b"<a x='1&amp;\n2'>ab\r\ncd]e",
            b"f<![CDATA[x]]y\n]]></a>",
            b"",
        ]);
        assert_eq!(e, 0);
        assert_eq!(
            ev,
            [
                "<a x=1& 2>",
                "c[ab]",
                "c[\n]",
                "c[cd]e]",
                "c[f]",
                "[[",
                "c[x]",
                "c[]]",
                "c[]y]",
                "c[\n]",
                "]]",
                "</a>"
            ]
        );
    }

    #[test]
    fn errors_and_lines() {
        assert_eq!(run(&[b"<a>\n<b></a>"]).1, ERROR_TAG_MISMATCH);
        assert_eq!(run(&[b"<a>\n<b></a>"]).2, 2);
        assert_eq!(run(&[b"<a/>x"]).1, ERROR_JUNK_AFTER_DOC_ELEMENT);
        assert_eq!(run(&[b"<a>\n\n"]).1, ERROR_NO_ELEMENTS);
        assert_eq!(run(&[b"<a>\n\n"]).2, 3);
        assert_eq!(run(&[b"<a b='1' b='2'/>"]).1, ERROR_DUPLICATE_ATTRIBUTE);
        assert_eq!(run(&[b"<a>&x;</a>"]).1, ERROR_UNDEFINED_ENTITY);
        assert_eq!(run(&[b"<a>&#0;</a>"]).1, ERROR_BAD_CHAR_REF);
        assert_eq!(run(&[b"<a>]]></a>"]).1, ERROR_INVALID_TOKEN);
        assert_eq!(run(&[b"<a><![CDATA[x"]).1, ERROR_UNCLOSED_CDATA_SECTION);
        assert_eq!(run(&[b"<a"]).1, ERROR_UNCLOSED_TOKEN);
        assert_eq!(run(&[b""]).1, ERROR_NO_ELEMENTS);
        assert_eq!(run(&[b"x<a/>"]).1, ERROR_SYNTAX);
        assert_eq!(error_string(ERROR_SYNTAX), Some("syntax error"));
        assert_eq!(error_string(0), None);
    }
}
