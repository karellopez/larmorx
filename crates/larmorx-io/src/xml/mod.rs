// SPDX-License-Identifier: Apache-2.0 AND MIT
//! A pure-Rust XML parser translated from expat 2.0.1 (Copyright (c) 1998, 1999, 2000 Thai Open
//! Source Software Center Ltd and Clark Cooper; Copyright (c) 2001-2006 Expat maintainers; MIT,
//! `PROVENANCE.md`), for gifticlib: the paths of a parser made by `XML_ParserCreate(NULL)` with
//! expat's own configuration as FreeSurfer builds it (`XML_DTD`, `XML_CONTEXT_BYTES` 1024), no
//! namespace processing, no external entities read, no parameter entities expanded, and
//! gifticlib's handlers (start and end tags, character data, processing instructions,
//! comments, CDATA sections, the default handler, the XML declaration, the document type and
//! element declarations).
//!
//! What a caller sees is expat's: the events and their pieces (character data split where
//! expat splits it, and, for documents in ISO-8859-1 or UTF-16, converted to UTF-8 in pieces of
//! at most 1024 bytes); the markup expat passes to the default handler (an internal subset's
//! declarations token by token, references to declared entities, white space in the prolog
//! and after the root element); attribute values normalized with their defaults from the
//! internal subset and its entities expanded; the error codes with [`error_string`]'s texts at
//! the line [`Parser::current_line_number`] gives. `validation/probes/fs-gifti` compares every
//! event and error with expat 2.0.1's and FreeSurfer 8.2.0's copy on generated documents.

// expat's functions report a status, the error kept in the parser: `Result<_, ()>` is that status.
#![allow(clippy::result_unit_err)]

mod nametab;
mod role;
mod tok;

use role::*;
use std::collections::{HashMap, HashSet};
use tok::*;

/// expat's `XML_Error` codes.
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
pub const ERROR_PARAM_ENTITY_REF: ErrorCode = 10;
pub const ERROR_UNDEFINED_ENTITY: ErrorCode = 11;
pub const ERROR_RECURSIVE_ENTITY_REF: ErrorCode = 12;
pub const ERROR_ASYNC_ENTITY: ErrorCode = 13;
pub const ERROR_BAD_CHAR_REF: ErrorCode = 14;
pub const ERROR_BINARY_ENTITY_REF: ErrorCode = 15;
pub const ERROR_ATTRIBUTE_EXTERNAL_ENTITY_REF: ErrorCode = 16;
pub const ERROR_MISPLACED_XML_PI: ErrorCode = 17;
pub const ERROR_UNKNOWN_ENCODING: ErrorCode = 18;
pub const ERROR_INCORRECT_ENCODING: ErrorCode = 19;
pub const ERROR_UNCLOSED_CDATA_SECTION: ErrorCode = 20;
pub const ERROR_UNEXPECTED_STATE: ErrorCode = 23;
pub const ERROR_ENTITY_DECLARED_IN_PE: ErrorCode = 24;
pub const ERROR_XML_DECL: ErrorCode = 30;
pub const ERROR_PUBLICID: ErrorCode = 32;
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

/// The content types of an element declaration (`XML_Content_Type`).
pub const CTYPE_EMPTY: i32 = 1;
pub const CTYPE_ANY: i32 = 2;
pub const CTYPE_MIXED: i32 = 3;
pub const CTYPE_NAME: i32 = 4;
pub const CTYPE_CHOICE: i32 = 5;
pub const CTYPE_SEQ: i32 = 6;

/// An element declaration's content model (`XML_Content`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Content {
    pub kind: i32,
    pub quant: i32,
    pub name: Option<Vec<u8>>,
    pub children: Vec<Content>,
}

/// The callbacks gifticlib registers; every one has an empty default. Strings are UTF-8.
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

/// `INIT_DATA_BUF_SIZE`: the piece size of converted character data and default text.
const DATA_BUF: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Proc {
    PrologInit,
    Prolog,
    Content,
    Cdata,
    Epilog,
    Error,
}

/// A general entity of the internal subset.
#[derive(Clone, Debug, Default)]
struct Entity {
    /// Its replacement text (internal UTF-8), for an internal entity.
    text: Option<Vec<u8>>,
    notation: bool,
    open: bool,
}

#[derive(Clone, Debug, Default)]
struct DefaultAtt {
    id: Vec<u8>,
    value: Option<Vec<u8>>,
    is_cdata: bool,
}

#[derive(Clone, Debug, Default)]
struct ElementType {
    default_atts: Vec<DefaultAtt>,
}

#[derive(Clone, Debug, Default)]
struct Scaffold {
    kind: i32,
    quant: i32,
    name: Vec<u8>,
    firstchild: usize,
    lastchild: usize,
    childcnt: usize,
    nextsib: usize,
}

/// The parts of expat's `DTD` the document entity reaches.
#[derive(Default)]
struct Dtd {
    keep_processing: bool,
    has_param_entity_refs: bool,
    standalone: bool,
    general: HashMap<Vec<u8>, Entity>,
    params: HashSet<Vec<u8>>,
    element_types: HashMap<Vec<u8>, ElementType>,
    /// Attribute names some declaration made non-CDATA (`maybeTokenized`).
    tokenized: HashSet<Vec<u8>>,
    in_eldecl: bool,
    scaffold: Vec<Scaffold>,
    scaff_index: Vec<usize>,
    scaff_level: usize,
}

/// The declaration being read (doProlog's `declEntity`, `declElementType`, ...).
#[derive(Default)]
struct Decl {
    entity: Option<(bool, Vec<u8>)>,
    element: Option<Vec<u8>>,
    attribute: Vec<u8>,
    attribute_is_cdata: bool,
    attribute_is_id: bool,
    element_decl_name: Vec<u8>,
}

/// The parser (`XML_Parser`).
pub struct Parser {
    /// The bytes not yet parsed (from `buffer_ptr`, absolute offset `base`).
    buf: Vec<u8>,
    base: usize,
    buffer_ptr: usize,
    position_ptr: usize,
    position: (u64, u64),
    event_ptr: Option<usize>,
    processor: Proc,
    error: ErrorCode,
    finished: bool,
    final_buffer: bool,
    /// `None`: the initial encoding, still to be detected.
    encoding: Option<Enc>,
    prolog: role::State,
    dtd: Dtd,
    decl: Decl,
    group_connector: Vec<u8>,
    /// The open elements: raw name (document encoding) and name (UTF-8).
    tags: Vec<(Vec<u8>, Vec<u8>)>,
    doctype_name: Option<Vec<u8>>,
    doctype_sysid: Option<Vec<u8>>,
    doctype_pubid: Option<Vec<u8>>,
    past_end: bool,
    line_past_end: bool,
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

/// A start tag's attributes: (name, value) in UTF-8.
type Attributes = Vec<(Vec<u8>, Vec<u8>)>;

/// A processor's or a scanner's result: `Ok(next)` (where parsing continues next time) or the
/// error.
type R = Result<usize, ErrorCode>;

impl Parser {
    /// `XML_ParserCreate(NULL)`.
    pub fn new() -> Parser {
        Parser {
            buf: Vec::new(),
            base: 0,
            buffer_ptr: 0,
            position_ptr: 0,
            position: (0, 0),
            event_ptr: None,
            processor: Proc::PrologInit,
            error: ERROR_NONE,
            finished: false,
            final_buffer: false,
            encoding: None,
            prolog: role::State::new(),
            dtd: Dtd {
                keep_processing: true,
                ..Dtd::default()
            },
            decl: Decl::default(),
            group_connector: Vec::new(),
            tags: Vec::new(),
            doctype_name: None,
            doctype_sysid: None,
            doctype_pubid: None,
            past_end: false,
            line_past_end: false,
        }
    }

    /// Whether expat 2.0.1's [`Parser::current_line_number`] would have read past the document:
    /// an error reported inside a multibyte character (a non-ASCII character in an XML
    /// declaration's pseudo-attribute), which its line count steps over (the program crashes).
    /// larmorx counts the lines before it.
    pub fn line_read_past_end(&self) -> bool {
        self.line_past_end
    }

    /// Whether expat 2.0.1 would have read past the end of the document: the last token of a
    /// UTF-16 document of an odd length (its last byte not a whole unit), which expat converts
    /// in a loop that steps over the end (the program crashes or never returns). larmorx drops
    /// the odd byte.
    pub fn read_past_end(&self) -> bool {
        self.past_end
    }

    /// `XML_GetErrorCode`.
    pub fn error_code(&self) -> ErrorCode {
        self.error
    }

    /// The encoding positions are counted in (the initial encoding counts as UTF-8).
    fn pos_enc(&self) -> Enc {
        self.encoding.unwrap_or(Enc::Utf8)
    }

    /// `XML_GetCurrentLineNumber`.
    pub fn current_line_number(&mut self) -> u64 {
        if let Some(e) = self.event_ptr
            && e >= self.position_ptr
        {
            let (b, p0, p1) = (self.base, self.position_ptr, e);
            let enc = self.pos_enc();
            let mut pos = self.position;
            if !enc.update_position(&self.buf, p0 - b, p1 - b, &mut pos) {
                self.line_past_end = true;
            }
            self.position = pos;
            self.position_ptr = e;
        }
        self.position.0 + 1
    }

    /// `XML_Parse`: parses `data` after what earlier calls left unparsed; `is_final` ends the
    /// document. `Err` after an error ([`Parser::error_code`]).
    pub fn parse(&mut self, data: &[u8], is_final: bool, h: &mut dyn Handler) -> Result<(), ()> {
        if self.finished {
            self.error = ERROR_FINISHED;
            return Err(());
        }
        if self.processor == Proc::Error {
            return Err(());
        }
        if data.is_empty() {
            self.final_buffer = is_final;
            if !is_final {
                return Ok(());
            }
            self.position_ptr = self.buffer_ptr;
            return match self.run(h) {
                Ok(next) => {
                    self.buffer_ptr = next;
                    self.finished = true;
                    Ok(())
                }
                Err(e) => self.fail(e),
            };
        }
        // XML_GetBuffer keeps the unparsed bytes (and context bytes: not observable)
        let keep = self.buffer_ptr - self.base;
        self.buf.drain(..keep);
        self.base = self.buffer_ptr;
        self.buf.extend_from_slice(data);
        // XML_ParseBuffer
        self.position_ptr = self.buffer_ptr;
        self.final_buffer = is_final;
        match self.run(h) {
            Ok(next) => {
                self.buffer_ptr = next;
                if is_final {
                    self.finished = true;
                    return Ok(());
                }
                let enc = self.pos_enc();
                let mut pos = self.position;
                let _ = enc.update_position(
                    &self.buf,
                    self.position_ptr - self.base,
                    next - self.base,
                    &mut pos,
                );
                self.position = pos;
                self.position_ptr = next;
                Ok(())
            }
            Err(e) => self.fail(e),
        }
    }

    fn fail(&mut self, e: ErrorCode) -> Result<(), ()> {
        self.error = e;
        self.processor = Proc::Error;
        Err(())
    }

    /// The processor on `buf[buffer_ptr..]`: where the next call starts.
    fn run(&mut self, h: &mut dyn Handler) -> R {
        let s = self.buffer_ptr - self.base;
        let end = self.buf.len();
        let r = match self.processor {
            Proc::PrologInit => {
                // initializeEncoding: no protocol encoding (XmlInitEncoding with NULL)
                self.processor = Proc::Prolog;
                self.prolog_processor(s, end, h)
            }
            Proc::Prolog => self.prolog_processor(s, end, h),
            Proc::Content => self.content_processor(s, end, h),
            Proc::Cdata => self.cdata_section_processor(s, end, h),
            Proc::Epilog => self.epilog_processor(s, end, h),
            Proc::Error => Err(self.error),
        };
        r.map(|n| n + self.base)
    }

    fn set_event(&mut self, p: usize) {
        self.event_ptr = Some(p + self.base);
    }

    fn have_more(&self) -> bool {
        !self.final_buffer
    }

    /// `reportDefault` of `buf[s..end]`.
    fn report_default(&mut self, enc: Enc, s: usize, end: usize, h: &mut dyn Handler) {
        if !enc.is_utf8() {
            let mut s = s;
            loop {
                let mut out = Vec::new();
                enc.to_utf8(&self.buf, &mut s, end, &mut out, DATA_BUF);
                h.default(&out);
                self.set_event(s);
                if s == end {
                    break;
                }
            }
        } else {
            h.default(&self.buf[s..end]);
        }
    }

    /// Character data `buf[s..next]` to the handler (converted in pieces when needed).
    fn report_chars(&mut self, enc: Enc, s: usize, next: usize, h: &mut dyn Handler) {
        if !enc.is_utf8() {
            let mut s = s;
            loop {
                let mut out = Vec::new();
                enc.to_utf8(&self.buf, &mut s, next, &mut out, DATA_BUF);
                h.characters(&out);
                if s == next {
                    break;
                }
                self.set_event(s);
            }
        } else {
            h.characters(&self.buf[s..next]);
        }
    }

    /// `reportProcessingInstruction`.
    fn report_pi(&mut self, enc: Enc, start: usize, end: usize, h: &mut dyn Handler) {
        let m = enc.mbpc();
        let start = start + 2 * m;
        let tem = start + enc.name_length(&self.buf, start);
        let target = enc.utf8(&self.buf, start, tem);
        let ds = enc.skip_s(&self.buf, tem);
        let data = normalize_lines(&enc.utf8(&self.buf, ds, end - 2 * m));
        h.processing_instruction(&target, &data);
    }

    /// `reportComment`.
    fn report_comment(&mut self, enc: Enc, start: usize, end: usize, h: &mut dyn Handler) {
        let m = enc.mbpc();
        let data = normalize_lines(&enc.utf8(&self.buf, start + 4 * m, end - 3 * m));
        h.comment(&data);
    }

    // ------------------------------------------------------------ the prolog

    /// `XmlPrologTok` with the parser's encoding (`initScan` while it is not yet known).
    fn prolog_tok(&mut self, s: usize, end: usize, next: &mut usize) -> i32 {
        match self.encoding {
            Some(enc) => enc.prolog_tok(&self.buf, s, end, next),
            None => self.init_scan(s, end, next),
        }
    }

    /// `initScan` for the document entity's prolog, no encoding given by the caller.
    fn init_scan(&mut self, p: usize, end: usize, next: &mut usize) -> i32 {
        let b = &self.buf;
        if p == end {
            return TOK_NONE;
        }
        if p + 1 == end {
            // XML_DTD: no early return for the prolog state
            if matches!(b[p], 0xFE | 0xFF | 0xEF | 0x00 | 0x3C) {
                return TOK_PARTIAL;
            }
        } else {
            match ((b[p] as u16) << 8) | b[p + 1] as u16 {
                0xFEFF => {
                    *next = p + 2;
                    self.encoding = Some(Enc::Big2);
                    return TOK_BOM;
                }
                0x3C00 => {
                    self.encoding = Some(Enc::Little2);
                    return Enc::Little2.prolog_tok(b, p, end, next);
                }
                0xFFFE => {
                    *next = p + 2;
                    self.encoding = Some(Enc::Little2);
                    return TOK_BOM;
                }
                0xEFBB => {
                    if p + 2 == end {
                        return TOK_PARTIAL;
                    }
                    if b[p + 2] == 0xBF {
                        *next = p + 3;
                        self.encoding = Some(Enc::Utf8);
                        return TOK_BOM;
                    }
                }
                _ => {
                    if b[p] == 0 {
                        self.encoding = Some(Enc::Big2);
                        return Enc::Big2.prolog_tok(b, p, end, next);
                    } else if b[p + 1] == 0 {
                        self.encoding = Some(Enc::Little2);
                        return Enc::Little2.prolog_tok(b, p, end, next);
                    }
                }
            }
        }
        self.encoding = Some(Enc::Utf8);
        Enc::Utf8.prolog_tok(&self.buf, p, end, next)
    }

    fn prolog_processor(&mut self, s: usize, end: usize, h: &mut dyn Handler) -> R {
        let mut next = s;
        let tok = self.prolog_tok(s, end, &mut next);
        self.do_prolog(s, end, tok, next, h)
    }

    /// `processXmlDecl` of the document's XML declaration `buf[s..next]`.
    fn process_xml_decl(
        &mut self,
        s: usize,
        next: usize,
        h: &mut dyn Handler,
    ) -> Result<(), ErrorCode> {
        let enc = self.encoding.expect("an encoding");
        let d = match parse_xml_decl(enc, &self.buf, s, next) {
            Ok(d) => d,
            Err(bad) => {
                self.set_event(bad);
                return Err(ERROR_XML_DECL);
            }
        };
        if d.standalone == 1 {
            self.dtd.standalone = true;
        }
        let enc_name = d
            .encoding_name
            .map(|p| enc.utf8(&self.buf, p, p + enc.name_length(&self.buf, p)));
        let version = d
            .version
            .map(|(a, e)| enc.utf8(&self.buf, a, e - enc.mbpc()));
        h.xml_decl(version.as_deref(), enc_name.as_deref(), d.standalone);
        match d.new_encoding {
            Some(ne) => {
                if ne.mbpc() != enc.mbpc() {
                    self.set_event(d.encoding_name.expect("a name"));
                    return Err(ERROR_INCORRECT_ENCODING);
                }
                self.encoding = Some(ne);
            }
            None => {
                if let Some(p) = d.encoding_name {
                    // handleUnknownEncoding without an unknown-encoding handler
                    self.set_event(p);
                    return Err(ERROR_UNKNOWN_ENCODING);
                }
            }
        }
        Ok(())
    }

    /// `doProlog`.
    fn do_prolog(
        &mut self,
        mut s: usize,
        end: usize,
        mut tok: i32,
        mut next: usize,
        h: &mut dyn Handler,
    ) -> R {
        loop {
            let mut handle_default = true;
            self.set_event(s);
            if tok <= 0 {
                if self.have_more() && tok != TOK_INVALID {
                    return Ok(s);
                }
                match tok {
                    TOK_INVALID => {
                        self.set_event(next);
                        return Err(ERROR_INVALID_TOKEN);
                    }
                    TOK_PARTIAL => return Err(ERROR_UNCLOSED_TOKEN),
                    TOK_PARTIAL_CHAR => return Err(ERROR_PARTIAL_CHAR),
                    TOK_NONE => return Err(ERROR_NO_ELEMENTS),
                    _ => {
                        tok = -tok;
                        next = end;
                        if self.encoding.is_some_and(|e| e.mbpc() == 2) && (end - s) % 2 == 1 {
                            self.past_end = true;
                        }
                    }
                }
            }
            let enc = self.encoding.unwrap_or(Enc::Utf8);
            let m = enc.mbpc();
            let role = self.prolog.role(tok, &self.buf, s, next, enc);
            match role {
                ROLE_XML_DECL => {
                    self.process_xml_decl(s, next, h)?;
                    handle_default = false;
                }
                ROLE_DOCTYPE_NAME => {
                    self.doctype_name = Some(enc.utf8(&self.buf, s, next));
                    self.doctype_pubid = None;
                    self.doctype_sysid = None;
                    handle_default = false;
                }
                ROLE_DOCTYPE_INTERNAL_SUBSET => {
                    let name = self.doctype_name.take().unwrap_or_default();
                    h.start_doctype(
                        &name,
                        self.doctype_sysid.as_deref(),
                        self.doctype_pubid.as_deref(),
                        true,
                    );
                    handle_default = false;
                }
                ROLE_DOCTYPE_PUBLIC_ID => {
                    self.dtd.has_param_entity_refs = true;
                    if let Err(bad) = enc.is_public_id(&self.buf, s, next) {
                        self.set_event(bad);
                        return Err(ERROR_PUBLICID);
                    }
                    let v = enc.utf8(&self.buf, s + m, next - m);
                    self.doctype_pubid = Some(normalize_public_id(&v));
                    handle_default = false;
                }
                ROLE_ENTITY_PUBLIC_ID => {
                    if let Err(bad) = enc.is_public_id(&self.buf, s, next) {
                        self.set_event(bad);
                        return Err(ERROR_PUBLICID);
                    }
                }
                ROLE_DOCTYPE_CLOSE => {
                    if let Some(name) = self.doctype_name.take() {
                        h.start_doctype(
                            &name,
                            self.doctype_sysid.as_deref(),
                            self.doctype_pubid.as_deref(),
                            false,
                        );
                    }
                    if self.doctype_sysid.is_some() {
                        self.dtd.has_param_entity_refs = true;
                    }
                    h.end_doctype();
                    handle_default = false;
                }
                ROLE_INSTANCE_START => {
                    self.processor = Proc::Content;
                    return self.content_processor(s, end, h);
                }
                ROLE_ATTLIST_ELEMENT_NAME => {
                    let name = enc.utf8(&self.buf, s, next);
                    self.dtd.element_types.entry(name.clone()).or_default();
                    self.decl.element = Some(name);
                }
                ROLE_ATTRIBUTE_NAME => {
                    self.decl.attribute = enc.utf8(&self.buf, s, next);
                    self.decl.attribute_is_cdata = false;
                    self.decl.attribute_is_id = false;
                }
                ROLE_ATTRIBUTE_TYPE_CDATA => self.decl.attribute_is_cdata = true,
                ROLE_ATTRIBUTE_TYPE_ID => self.decl.attribute_is_id = true,
                ROLE_IMPLIED_ATTRIBUTE_VALUE | ROLE_REQUIRED_ATTRIBUTE_VALUE => {
                    if self.dtd.keep_processing {
                        let is_id = self.decl.attribute_is_id;
                        self.define_attribute(None, is_id);
                    }
                }
                ROLE_DEFAULT_ATTRIBUTE_VALUE | ROLE_FIXED_ATTRIBUTE_VALUE => {
                    if self.dtd.keep_processing {
                        let is_cdata = self.decl.attribute_is_cdata;
                        let v = self.store_attribute_value(enc, is_cdata, s + m, next - m, true)?;
                        self.define_attribute(Some(v), false);
                    }
                }
                ROLE_ENTITY_VALUE => {
                    if self.dtd.keep_processing {
                        let (r, text) = self.store_entity_value(enc, s + m, next - m);
                        if let Some((false, name)) = &self.decl.entity
                            && let Some(e) = self.dtd.general.get_mut(name)
                        {
                            e.text = Some(text);
                        }
                        r?;
                    }
                }
                ROLE_DOCTYPE_SYSTEM_ID => {
                    self.dtd.has_param_entity_refs = true;
                    self.doctype_sysid = Some(enc.utf8(&self.buf, s + m, next - m));
                    handle_default = false;
                }
                ROLE_ENTITY_NOTATION_NAME => {
                    if self.dtd.keep_processing
                        && let Some((false, name)) = &self.decl.entity
                        && let Some(e) = self.dtd.general.get_mut(name)
                    {
                        e.notation = true;
                    }
                }
                ROLE_GENERAL_ENTITY_NAME => {
                    if enc.predefined_entity_name(&self.buf, s, next) != 0 {
                        self.decl.entity = None;
                    } else if self.dtd.keep_processing {
                        let name = enc.utf8(&self.buf, s, next);
                        if self.dtd.general.contains_key(&name) {
                            self.decl.entity = None;
                        } else {
                            self.dtd.general.insert(name.clone(), Entity::default());
                            self.decl.entity = Some((false, name));
                        }
                    } else {
                        self.decl.entity = None;
                    }
                }
                ROLE_PARAM_ENTITY_NAME => {
                    if self.dtd.keep_processing {
                        let name = enc.utf8(&self.buf, s, next);
                        if self.dtd.params.insert(name.clone()) {
                            self.decl.entity = Some((true, name));
                        } else {
                            self.decl.entity = None;
                        }
                    } else {
                        self.decl.entity = None;
                    }
                }
                ROLE_NOTATION_PUBLIC_ID => {
                    if let Err(bad) = enc.is_public_id(&self.buf, s, next) {
                        self.set_event(bad);
                        return Err(ERROR_PUBLICID);
                    }
                }
                ROLE_ERROR => {
                    return Err(match tok {
                        TOK_PARAM_ENTITY_REF => ERROR_PARAM_ENTITY_REF,
                        TOK_XML_DECL => ERROR_MISPLACED_XML_PI,
                        _ => ERROR_SYNTAX,
                    });
                }
                ROLE_GROUP_OPEN => {
                    let level = self.prolog.level as usize;
                    if self.group_connector.len() <= level {
                        self.group_connector.resize(level + 1, 0);
                    }
                    self.group_connector[level] = 0;
                    if self.dtd.in_eldecl {
                        let me = self.next_scaffold_part();
                        let lvl = self.dtd.scaff_level;
                        if self.dtd.scaff_index.len() <= lvl {
                            self.dtd.scaff_index.resize(lvl + 1, 0);
                        }
                        self.dtd.scaff_index[lvl] = me;
                        self.dtd.scaff_level += 1;
                        self.dtd.scaffold[me].kind = CTYPE_SEQ;
                        handle_default = false;
                    }
                }
                ROLE_GROUP_SEQUENCE => {
                    let level = self.prolog.level as usize;
                    if self.group_connector[level] == b'|' {
                        return Err(ERROR_SYNTAX);
                    }
                    self.group_connector[level] = b',';
                    if self.dtd.in_eldecl {
                        handle_default = false;
                    }
                }
                ROLE_GROUP_CHOICE => {
                    let level = self.prolog.level as usize;
                    if self.group_connector[level] == b',' {
                        return Err(ERROR_SYNTAX);
                    }
                    if self.dtd.in_eldecl && self.group_connector[level] == 0 {
                        let cur = self.dtd.scaff_index[self.dtd.scaff_level - 1];
                        if self.dtd.scaffold[cur].kind != CTYPE_MIXED {
                            self.dtd.scaffold[cur].kind = CTYPE_CHOICE;
                            handle_default = false;
                        }
                    }
                    self.group_connector[level] = b'|';
                }
                ROLE_PARAM_ENTITY_REF => {
                    // no parameter entity parsing: what follows is not processed unless the
                    // document is standalone
                    self.dtd.has_param_entity_refs = true;
                    self.dtd.keep_processing = self.dtd.standalone;
                }
                ROLE_ELEMENT_NAME => {
                    let name = enc.utf8(&self.buf, s, next);
                    self.dtd.element_types.entry(name.clone()).or_default();
                    self.decl.element_decl_name = name;
                    self.dtd.scaff_level = 0;
                    self.dtd.scaffold.clear();
                    self.dtd.in_eldecl = true;
                    handle_default = false;
                }
                ROLE_CONTENT_ANY | ROLE_CONTENT_EMPTY => {
                    if self.dtd.in_eldecl {
                        let model = Content {
                            kind: if role == ROLE_CONTENT_ANY {
                                CTYPE_ANY
                            } else {
                                CTYPE_EMPTY
                            },
                            quant: 0,
                            name: None,
                            children: Vec::new(),
                        };
                        let name = self.decl.element_decl_name.clone();
                        h.element_decl(&name, &model);
                        handle_default = false;
                        self.dtd.in_eldecl = false;
                    }
                }
                ROLE_CONTENT_PCDATA => {
                    if self.dtd.in_eldecl {
                        let cur = self.dtd.scaff_index[self.dtd.scaff_level - 1];
                        self.dtd.scaffold[cur].kind = CTYPE_MIXED;
                        handle_default = false;
                    }
                }
                ROLE_CONTENT_ELEMENT
                | ROLE_CONTENT_ELEMENT_OPT
                | ROLE_CONTENT_ELEMENT_REP
                | ROLE_CONTENT_ELEMENT_PLUS => {
                    if self.dtd.in_eldecl {
                        let quant = match role {
                            ROLE_CONTENT_ELEMENT => 0,
                            ROLE_CONTENT_ELEMENT_OPT => 1,
                            ROLE_CONTENT_ELEMENT_REP => 2,
                            _ => 3,
                        };
                        let nxt = if quant == 0 { next } else { next - m };
                        let me = self.next_scaffold_part();
                        let name = enc.utf8(&self.buf, s, nxt);
                        self.dtd.element_types.entry(name.clone()).or_default();
                        let sc = &mut self.dtd.scaffold[me];
                        sc.kind = CTYPE_NAME;
                        sc.quant = quant;
                        sc.name = name;
                        handle_default = false;
                    }
                }
                ROLE_GROUP_CLOSE
                | ROLE_GROUP_CLOSE_OPT
                | ROLE_GROUP_CLOSE_REP
                | ROLE_GROUP_CLOSE_PLUS => {
                    if self.dtd.in_eldecl {
                        let quant = match role {
                            ROLE_GROUP_CLOSE => 0,
                            ROLE_GROUP_CLOSE_OPT => 1,
                            ROLE_GROUP_CLOSE_REP => 2,
                            _ => 3,
                        };
                        handle_default = false;
                        self.dtd.scaff_level -= 1;
                        let at = self.dtd.scaff_index[self.dtd.scaff_level];
                        self.dtd.scaffold[at].quant = quant;
                        if self.dtd.scaff_level == 0 {
                            let model = self.build_node(0);
                            let name = self.decl.element_decl_name.clone();
                            h.element_decl(&name, &model);
                            self.dtd.in_eldecl = false;
                        }
                    }
                }
                ROLE_PI => {
                    self.report_pi(enc, s, next, h);
                    handle_default = false;
                }
                ROLE_COMMENT => {
                    self.report_comment(enc, s, next, h);
                    handle_default = false;
                }
                ROLE_NONE => {
                    if tok == TOK_BOM {
                        handle_default = false;
                    }
                }
                ROLE_DOCTYPE_NONE | ROLE_ELEMENT_NONE => handle_default = false,
                _ => {}
            }
            if handle_default {
                let enc = self.encoding.unwrap_or(Enc::Utf8);
                self.report_default(enc, s, next, h);
            }
            s = next;
            tok = self.prolog_tok(s, end, &mut next);
        }
    }

    /// `defineAttribute` of the attribute being declared.
    fn define_attribute(&mut self, value: Option<Vec<u8>>, is_id: bool) {
        let el = self.decl.element.clone().unwrap_or_default();
        let id = self.decl.attribute.clone();
        let is_cdata = self.decl.attribute_is_cdata;
        let t = self.dtd.element_types.entry(el).or_default();
        if (value.is_some() || is_id) && t.default_atts.iter().any(|d| d.id == id) {
            return;
        }
        t.default_atts.push(DefaultAtt {
            id: id.clone(),
            value,
            is_cdata,
        });
        if !is_cdata {
            self.dtd.tokenized.insert(id);
        }
    }

    /// `nextScaffoldPart`.
    fn next_scaffold_part(&mut self) -> usize {
        let next = self.dtd.scaffold.len();
        self.dtd.scaffold.push(Scaffold::default());
        if self.dtd.scaff_level > 0 {
            let parent = self.dtd.scaff_index[self.dtd.scaff_level - 1];
            let last = self.dtd.scaffold[parent].lastchild;
            if last != 0 {
                self.dtd.scaffold[last].nextsib = next;
            }
            let p = &mut self.dtd.scaffold[parent];
            if p.childcnt == 0 {
                p.firstchild = next;
            }
            p.lastchild = next;
            p.childcnt += 1;
        }
        next
    }

    /// `build_node`.
    fn build_node(&self, src: usize) -> Content {
        let sc = &self.dtd.scaffold[src];
        if sc.kind == CTYPE_NAME {
            return Content {
                kind: sc.kind,
                quant: sc.quant,
                name: Some(sc.name.clone()),
                children: Vec::new(),
            };
        }
        let mut children = Vec::with_capacity(sc.childcnt);
        let mut cn = sc.firstchild;
        for _ in 0..sc.childcnt {
            children.push(self.build_node(cn));
            cn = self.dtd.scaffold[cn].nextsib;
        }
        Content {
            kind: sc.kind,
            quant: sc.quant,
            name: None,
            children,
        }
    }

    /// `storeEntityValue`: its status and the value (internal UTF-8).
    fn store_entity_value(
        &mut self,
        enc: Enc,
        mut p: usize,
        end: usize,
    ) -> (Result<(), ErrorCode>, Vec<u8>) {
        let mut v = Vec::new();
        loop {
            let mut next = p;
            let tok = enc.entity_value_tok(&self.buf, p, end, &mut next);
            match tok {
                TOK_PARAM_ENTITY_REF => {
                    self.set_event(p);
                    return (Err(ERROR_PARAM_ENTITY_REF), v);
                }
                TOK_NONE => return (Ok(()), v),
                TOK_ENTITY_REF | TOK_DATA_CHARS => {
                    let mut f = p;
                    enc.to_utf8(&self.buf, &mut f, next, &mut v, usize::MAX);
                }
                TOK_TRAILING_CR | TOK_DATA_NEWLINE => {
                    if tok == TOK_TRAILING_CR {
                        next = p + enc.mbpc();
                    }
                    v.push(0x0A);
                }
                TOK_CHAR_REF => {
                    let n = enc.char_ref_number(&self.buf, p);
                    if n < 0 || utf8_encode(n, &mut v) == 0 {
                        self.set_event(p);
                        return (Err(ERROR_BAD_CHAR_REF), v);
                    }
                }
                TOK_PARTIAL => {
                    self.set_event(p);
                    return (Err(ERROR_INVALID_TOKEN), v);
                }
                TOK_INVALID => {
                    self.set_event(next);
                    return (Err(ERROR_INVALID_TOKEN), v);
                }
                _ => {
                    self.set_event(p);
                    return (Err(ERROR_UNEXPECTED_STATE), v);
                }
            }
            p = next;
        }
    }

    /// `storeAttributeValue` of `bytes[p..end]` (the document's buffer, or an entity's text
    /// in the internal encoding); `prolog`: a default value in the internal subset.
    fn store_attribute_value(
        &mut self,
        enc: Enc,
        is_cdata: bool,
        p: usize,
        end: usize,
        prolog: bool,
    ) -> Result<Vec<u8>, ErrorCode> {
        let mut pool = Vec::new();
        let buf = std::mem::take(&mut self.buf);
        let r = self.append_attribute_value(enc, true, is_cdata, &buf, p, end, &mut pool, prolog);
        self.buf = buf;
        r?;
        if !is_cdata && pool.last() == Some(&0x20) {
            pool.pop();
        }
        Ok(pool)
    }

    /// `appendAttributeValue`; `doc`: the bytes are the document's (positions are recorded).
    #[allow(clippy::too_many_arguments)]
    fn append_attribute_value(
        &mut self,
        enc: Enc,
        doc: bool,
        is_cdata: bool,
        b: &[u8],
        mut p: usize,
        end: usize,
        pool: &mut Vec<u8>,
        prolog: bool,
    ) -> Result<(), ErrorCode> {
        loop {
            let mut next = p;
            let tok = enc.attribute_value_tok(b, p, end, &mut next);
            match tok {
                TOK_NONE => return Ok(()),
                TOK_INVALID => {
                    if doc {
                        self.set_event(next);
                    }
                    return Err(ERROR_INVALID_TOKEN);
                }
                TOK_PARTIAL => {
                    if doc {
                        self.set_event(p);
                    }
                    return Err(ERROR_INVALID_TOKEN);
                }
                TOK_CHAR_REF => {
                    let n = enc.char_ref_number(b, p);
                    if n < 0 {
                        if doc {
                            self.set_event(p);
                        }
                        return Err(ERROR_BAD_CHAR_REF);
                    }
                    if !is_cdata && n == 0x20 && (pool.is_empty() || pool.last() == Some(&0x20)) {
                        p = next;
                        continue;
                    }
                    if utf8_encode(n, pool) == 0 {
                        if doc {
                            self.set_event(p);
                        }
                        return Err(ERROR_BAD_CHAR_REF);
                    }
                }
                TOK_DATA_CHARS => {
                    let mut f = p;
                    enc.to_utf8(b, &mut f, next, pool, usize::MAX);
                }
                TOK_TRAILING_CR | TOK_ATTRIBUTE_VALUE_S | TOK_DATA_NEWLINE => {
                    if tok == TOK_TRAILING_CR {
                        next = p + enc.mbpc();
                    }
                    if !(!is_cdata && (pool.is_empty() || pool.last() == Some(&0x20))) {
                        pool.push(0x20);
                    }
                }
                TOK_ENTITY_REF => {
                    let m = enc.mbpc();
                    let ch = enc.predefined_entity_name(b, p + m, next - m);
                    if ch != 0 {
                        pool.push(ch);
                        p = next;
                        continue;
                    }
                    let name = enc.utf8(b, p + m, next - m);
                    let check = if prolog {
                        // documentEntity && (standalone ? !openInternalEntities : !hasParamEntityRefs)
                        self.dtd.standalone || !self.dtd.has_param_entity_refs
                    } else {
                        !self.dtd.has_param_entity_refs || self.dtd.standalone
                    };
                    let Some(entity) = self.dtd.general.get(&name).cloned() else {
                        if check {
                            return Err(ERROR_UNDEFINED_ENTITY);
                        }
                        p = next;
                        continue;
                    };
                    if entity.open {
                        if doc {
                            self.set_event(p);
                        }
                        return Err(ERROR_RECURSIVE_ENTITY_REF);
                    }
                    if entity.notation {
                        if doc {
                            self.set_event(p);
                        }
                        return Err(ERROR_BINARY_ENTITY_REF);
                    }
                    let Some(text) = entity.text else {
                        if doc {
                            self.set_event(p);
                        }
                        return Err(ERROR_ATTRIBUTE_EXTERNAL_ENTITY_REF);
                    };
                    self.set_open(&name, true);
                    let r = self.append_attribute_value(
                        Enc::Internal,
                        false,
                        is_cdata,
                        &text,
                        0,
                        text.len(),
                        pool,
                        prolog,
                    );
                    self.set_open(&name, false);
                    r?;
                }
                _ => {
                    if doc {
                        self.set_event(p);
                    }
                    return Err(ERROR_UNEXPECTED_STATE);
                }
            }
            p = next;
        }
    }

    fn set_open(&mut self, name: &[u8], open: bool) {
        if let Some(e) = self.dtd.general.get_mut(name) {
            e.open = open;
        }
    }

    // ------------------------------------------------------------ the content

    fn content_processor(&mut self, s: usize, end: usize, h: &mut dyn Handler) -> R {
        self.do_content(s, end, h)
    }

    /// `doContent` of the document entity (`startTagLevel` 0).
    fn do_content(&mut self, mut s: usize, end: usize, h: &mut dyn Handler) -> R {
        let enc = self.encoding.expect("an encoding");
        let m = enc.mbpc();
        self.set_event(s);
        loop {
            let mut next = s;
            let tok = enc.content_tok(&self.buf, s, end, &mut next);
            match tok {
                TOK_TRAILING_CR => {
                    if self.have_more() {
                        return Ok(s);
                    }
                    h.characters(b"\n");
                    return Err(ERROR_NO_ELEMENTS);
                }
                TOK_NONE => {
                    if self.have_more() {
                        return Ok(s);
                    }
                    return Err(ERROR_NO_ELEMENTS);
                }
                TOK_INVALID => {
                    self.set_event(next);
                    return Err(ERROR_INVALID_TOKEN);
                }
                TOK_PARTIAL => {
                    if self.have_more() {
                        return Ok(s);
                    }
                    return Err(ERROR_UNCLOSED_TOKEN);
                }
                TOK_PARTIAL_CHAR => {
                    if self.have_more() {
                        return Ok(s);
                    }
                    return Err(ERROR_PARTIAL_CHAR);
                }
                TOK_ENTITY_REF => {
                    let ch = enc.predefined_entity_name(&self.buf, s + m, next - m);
                    if ch != 0 {
                        h.characters(&[ch]);
                    } else {
                        let name = enc.utf8(&self.buf, s + m, next - m);
                        let entity = self.dtd.general.get(&name).cloned();
                        if (!self.dtd.has_param_entity_refs || self.dtd.standalone)
                            && entity.is_none()
                        {
                            return Err(ERROR_UNDEFINED_ENTITY);
                        }
                        match entity {
                            None => self.report_default(enc, s, next, h),
                            Some(e) => {
                                if e.open {
                                    return Err(ERROR_RECURSIVE_ENTITY_REF);
                                }
                                if e.notation {
                                    return Err(ERROR_BINARY_ENTITY_REF);
                                }
                                // internal or external: not expanded (a default handler is set)
                                self.report_default(enc, s, next, h);
                            }
                        }
                    }
                }
                TOK_START_TAG_NO_ATTS | TOK_START_TAG_WITH_ATTS => {
                    let raw = s + m;
                    let len = enc.name_length(&self.buf, raw);
                    let name = enc.utf8(&self.buf, raw, raw + len);
                    self.tags
                        .push((self.buf[raw..raw + len].to_vec(), name.clone()));
                    let atts = self.store_atts(enc, s, &name)?;
                    h.start_element(&name, &atts);
                }
                TOK_EMPTY_ELEMENT_NO_ATTS | TOK_EMPTY_ELEMENT_WITH_ATTS => {
                    let raw = s + m;
                    let len = enc.name_length(&self.buf, raw);
                    let name = enc.utf8(&self.buf, raw, raw + len);
                    let atts = self.store_atts(enc, s, &name)?;
                    h.start_element(&name, &atts);
                    self.set_event(next);
                    h.end_element(&name);
                    if self.tags.is_empty() {
                        return self.epilog_processor(next, end, h);
                    }
                }
                TOK_END_TAG => {
                    let raw = s + 2 * m;
                    let len = enc.name_length(&self.buf, raw);
                    let (traw, tname) = self.tags.pop().expect("an open element");
                    if len != traw.len() || self.buf[raw..raw + len] != traw[..] {
                        self.set_event(raw);
                        return Err(ERROR_TAG_MISMATCH);
                    }
                    h.end_element(&tname);
                    if self.tags.is_empty() {
                        return self.epilog_processor(next, end, h);
                    }
                }
                TOK_CHAR_REF => {
                    let n = enc.char_ref_number(&self.buf, s);
                    if n < 0 {
                        return Err(ERROR_BAD_CHAR_REF);
                    }
                    let mut v = Vec::new();
                    utf8_encode(n, &mut v);
                    h.characters(&v);
                }
                TOK_XML_DECL => return Err(ERROR_MISPLACED_XML_PI),
                TOK_DATA_NEWLINE => h.characters(b"\n"),
                TOK_CDATA_SECT_OPEN => {
                    h.start_cdata();
                    match self.do_cdata_section(next, end, h)? {
                        Cdata::Closed(n) => next = n,
                        Cdata::Wait(at) => {
                            self.processor = Proc::Cdata;
                            return Ok(at);
                        }
                    }
                }
                TOK_TRAILING_RSQB => {
                    if self.have_more() {
                        return Ok(s);
                    }
                    if m == 2 && (end - s) % 2 == 1 {
                        self.past_end = true;
                    }
                    self.report_chars(enc, s, end, h);
                    self.set_event(end);
                    return Err(ERROR_NO_ELEMENTS);
                }
                TOK_DATA_CHARS => self.report_chars(enc, s, next, h),
                TOK_PI => self.report_pi(enc, s, next, h),
                TOK_COMMENT => self.report_comment(enc, s, next, h),
                _ => self.report_default(enc, s, next, h),
            }
            s = next;
            self.set_event(s);
        }
    }

    /// `storeAtts` for a parser without namespaces: the attributes for the start handler
    /// (specified ones, then the declared defaults not specified).
    fn store_atts(&mut self, enc: Enc, s: usize, name: &[u8]) -> Result<Attributes, ErrorCode> {
        self.dtd.element_types.entry(name.to_vec()).or_default();
        let defaults = self.dtd.element_types[name].default_atts.clone();
        let atts = enc.get_atts(&self.buf, s);
        let mut out: Vec<(Vec<u8>, Vec<u8>)> = Vec::with_capacity(atts.len());
        let mut seen: HashSet<Vec<u8>> = HashSet::new();
        for a in &atts {
            let an = enc.utf8(
                &self.buf,
                a.name,
                a.name + enc.name_length(&self.buf, a.name),
            );
            if !seen.insert(an.clone()) {
                self.set_event(a.name);
                return Err(ERROR_DUPLICATE_ATTRIBUTE);
            }
            let value = if !a.normalized {
                let mut is_cdata = true;
                if self.dtd.tokenized.contains(&an)
                    && let Some(d) = defaults.iter().find(|d| d.id == an)
                {
                    is_cdata = d.is_cdata;
                }
                self.store_attribute_value(enc, is_cdata, a.value, a.value_end, false)?
            } else {
                enc.utf8(&self.buf, a.value, a.value_end)
            };
            out.push((an, value));
        }
        for d in &defaults {
            if let Some(v) = &d.value
                && !seen.contains(&d.id)
            {
                seen.insert(d.id.clone());
                out.push((d.id.clone(), v.clone()));
            }
        }
        Ok(out)
    }

    fn cdata_section_processor(&mut self, s: usize, end: usize, h: &mut dyn Handler) -> R {
        match self.do_cdata_section(s, end, h)? {
            Cdata::Closed(next) => {
                self.processor = Proc::Content;
                self.content_processor(next, end, h)
            }
            Cdata::Wait(at) => Ok(at),
        }
    }

    /// `doCdataSection`.
    fn do_cdata_section(
        &mut self,
        mut s: usize,
        end: usize,
        h: &mut dyn Handler,
    ) -> Result<Cdata, ErrorCode> {
        let enc = self.encoding.expect("an encoding");
        self.set_event(s);
        loop {
            let mut next = s;
            let tok = enc.cdata_section_tok(&self.buf, s, end, &mut next);
            match tok {
                TOK_CDATA_SECT_CLOSE => {
                    h.end_cdata();
                    return Ok(Cdata::Closed(next));
                }
                TOK_DATA_NEWLINE => h.characters(b"\n"),
                TOK_DATA_CHARS => self.report_chars(enc, s, next, h),
                TOK_INVALID => {
                    self.set_event(next);
                    return Err(ERROR_INVALID_TOKEN);
                }
                TOK_PARTIAL_CHAR => {
                    if self.have_more() {
                        return Ok(Cdata::Wait(s));
                    }
                    return Err(ERROR_PARTIAL_CHAR);
                }
                TOK_PARTIAL | TOK_NONE => {
                    if self.have_more() {
                        return Ok(Cdata::Wait(s));
                    }
                    return Err(ERROR_UNCLOSED_CDATA_SECTION);
                }
                _ => {
                    self.set_event(next);
                    return Err(ERROR_UNEXPECTED_STATE);
                }
            }
            s = next;
            self.set_event(s);
        }
    }

    // ------------------------------------------------------------ after the root element

    /// `epilogProcessor`.
    fn epilog_processor(&mut self, mut s: usize, end: usize, h: &mut dyn Handler) -> R {
        self.processor = Proc::Epilog;
        let enc = self.encoding.expect("an encoding");
        self.set_event(s);
        loop {
            let mut next = s;
            let tok = enc.prolog_tok(&self.buf, s, end, &mut next);
            match tok {
                t if t == -TOK_PROLOG_S => {
                    self.report_default(enc, s, next, h);
                    return Ok(next);
                }
                TOK_NONE => return Ok(s),
                TOK_PROLOG_S => self.report_default(enc, s, next, h),
                TOK_PI => self.report_pi(enc, s, next, h),
                TOK_COMMENT => self.report_comment(enc, s, next, h),
                TOK_INVALID => {
                    self.set_event(next);
                    return Err(ERROR_INVALID_TOKEN);
                }
                TOK_PARTIAL => {
                    if self.have_more() {
                        return Ok(s);
                    }
                    return Err(ERROR_UNCLOSED_TOKEN);
                }
                TOK_PARTIAL_CHAR => {
                    if self.have_more() {
                        return Ok(s);
                    }
                    return Err(ERROR_PARTIAL_CHAR);
                }
                _ => return Err(ERROR_JUNK_AFTER_DOC_ELEMENT),
            }
            s = next;
            self.set_event(s);
        }
    }
}

/// A CDATA section's state after `doCdataSection`: closed (parsing goes on after it) or
/// waiting for more data (parsing restarts at the incomplete token).
enum Cdata {
    Closed(usize),
    Wait(usize),
}

/// The XML declaration's parts (`XmlParseXmlDecl`'s outputs).
struct XmlDecl {
    version: Option<(usize, usize)>,
    encoding_name: Option<usize>,
    new_encoding: Option<Enc>,
    standalone: i32,
}

fn is_space(c: i32) -> bool {
    matches!(c, 0x20 | 0x0D | 0x0A | 0x09)
}

/// `parsePseudoAttribute`: `Ok(None)` for nothing but white space, `Ok(Some((name, name end,
/// value)))` and the position after it, or `Err(bad position)`.
#[allow(clippy::type_complexity)]
fn parse_pseudo_attribute(
    enc: Enc,
    b: &[u8],
    mut p: usize,
    end: usize,
) -> Result<(Option<(usize, usize, usize)>, usize), usize> {
    let m = enc.mbpc();
    // nothing but white space: the caller's position does not move
    let p0 = p;
    if p == end {
        return Ok((None, p0));
    }
    if !is_space(enc.to_ascii(b, p, end)) {
        return Err(p);
    }
    loop {
        p += m;
        if !is_space(enc.to_ascii(b, p, end)) {
            break;
        }
    }
    if p == end {
        return Ok((None, p0));
    }
    let name = p;
    let name_end;
    loop {
        let c = enc.to_ascii(b, p, end);
        if c == -1 {
            return Err(p);
        }
        if c == b'=' as i32 {
            name_end = p;
            break;
        }
        if is_space(c) {
            name_end = p;
            let mut c;
            loop {
                p += m;
                c = enc.to_ascii(b, p, end);
                if !is_space(c) {
                    break;
                }
            }
            if c != b'=' as i32 {
                return Err(p);
            }
            break;
        }
        p += m;
    }
    if p == name {
        return Err(p);
    }
    p += m;
    let mut c = enc.to_ascii(b, p, end);
    while is_space(c) {
        p += m;
        c = enc.to_ascii(b, p, end);
    }
    if c != b'"' as i32 && c != b'\'' as i32 {
        return Err(p);
    }
    let open = c;
    p += m;
    let val = p;
    loop {
        let c = enc.to_ascii(b, p, end);
        if c == open {
            break;
        }
        let ok = (0x61..=0x7a).contains(&c)
            || (0x41..=0x5a).contains(&c)
            || (0x30..=0x39).contains(&c)
            || c == b'.' as i32
            || c == b'-' as i32
            || c == b'_' as i32;
        if !ok {
            return Err(p);
        }
        p += m;
    }
    Ok((Some((name, name_end, val)), p + m))
}

/// `XmlParseXmlDecl` of the document entity's declaration `b[s..next]`.
fn parse_xml_decl(enc: Enc, b: &[u8], s: usize, next: usize) -> Result<XmlDecl, usize> {
    let m = enc.mbpc();
    let p = s + 5 * m;
    let end = next - 2 * m;
    let mut d = XmlDecl {
        version: None,
        encoding_name: None,
        new_encoding: None,
        standalone: -1,
    };
    let (attr, mut p) = parse_pseudo_attribute(enc, b, p, end)?;
    let Some((mut name, mut name_end, mut val)) = attr else {
        return Err(p);
    };
    if !enc.name_matches_ascii(b, name, name_end, b"version") {
        return Err(name);
    }
    d.version = Some((val, p));
    let (attr, q) = parse_pseudo_attribute(enc, b, p, end)?;
    p = q;
    match attr {
        None => return Ok(d),
        Some((n, ne, v)) => {
            name = n;
            name_end = ne;
            val = v;
        }
    }
    if enc.name_matches_ascii(b, name, name_end, b"encoding") {
        let c = enc.to_ascii(b, val, end);
        if !((0x61..=0x7a).contains(&c) || (0x41..=0x5a).contains(&c)) {
            return Err(val);
        }
        d.encoding_name = Some(val);
        d.new_encoding = find_encoding(enc, b, val, p - m);
        let (attr, q) = parse_pseudo_attribute(enc, b, p, end)?;
        p = q;
        match attr {
            None => return Ok(d),
            Some((n, ne, v)) => {
                name = n;
                name_end = ne;
                val = v;
            }
        }
    }
    if !enc.name_matches_ascii(b, name, name_end, b"standalone") {
        return Err(name);
    }
    if enc.name_matches_ascii(b, val, p - m, b"yes") {
        d.standalone = 1;
    } else if enc.name_matches_ascii(b, val, p - m, b"no") {
        d.standalone = 0;
    } else {
        return Err(val);
    }
    while is_space(enc.to_ascii(b, p, end)) {
        p += m;
    }
    if p != end {
        return Err(p);
    }
    Ok(d)
}

/// `findEncoding` of the declared name `b[p..end]`.
fn find_encoding(enc: Enc, b: &[u8], p: usize, end: usize) -> Option<Enc> {
    let mut buf = Vec::new();
    let mut f = p;
    enc.to_utf8(b, &mut f, end, &mut buf, 127);
    if f != end {
        return None;
    }
    if buf.eq_ignore_ascii_case(b"UTF-16") && enc.mbpc() == 2 {
        return Some(enc);
    }
    encoding_index(&buf).map(|i| ENCODINGS[i])
}

/// `normalizeLines`: CR and CR LF to LF.
fn normalize_lines(s: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if s[i] == 0x0D {
            out.push(0x0A);
            i += 1;
            if i < s.len() && s[i] == 0x0A {
                i += 1;
            }
        } else {
            out.push(s[i]);
            i += 1;
        }
    }
    out
}

/// `normalizePublicId`.
fn normalize_public_id(s: &[u8]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity(s.len());
    for &c in s {
        match c {
            0x20 | 0x0D | 0x0A => {
                if !out.is_empty() && out.last() != Some(&0x20) {
                    out.push(0x20);
                }
            }
            _ => out.push(c),
        }
    }
    if out.last() == Some(&0x20) {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests;
