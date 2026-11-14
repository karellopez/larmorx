// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's XML help renderer (`utils/fsPrintHelp.cpp`: `outputHelpXml`, `outputHelp`,
//! `outputHelpDoc`), which prints a program's `.help.xml` text as its `--help`, and the XML reading
//! it needs: a pure-Rust reader that gives the document tree libxml2 2.7.7 (FreeSurfer's
//! `packages/xml2`, `xmlParseMemory`'s defaults: entities not substituted, blank text kept, no DTD
//! loaded) builds for a well-formed document (the CLAUDE.md "Parsers" rule: the same values on the
//! paths the programs reach, whose help texts are well formed).
//!
//! What the reader models: the XML declaration (version, encoding UTF-8, ISO-8859-1 or ASCII,
//! standalone), a byte-order mark, comments and processing instructions, the document type with
//! an internal subset of general entity declarations (the help files redeclare `amp`, `gt` and
//! `lt`; the predefined entities override any declaration, as in libxml2) and element, attribute
//! list and notation declarations (read to their end, their grammar not checked: with blanks
//! kept they change no text, and nothing reads them), elements with
//! attributes, character data with `\r\n` and `\r` read as `\n`, character references, the five
//! predefined entities, CDATA sections. What it does not model ([`Parsed::Unsupported`]):
//! parameter entities, references to declared entities,
//! names with a colon (namespaces) or outside ASCII, other encodings. A document that is not well
//! formed is [`Parsed::Malformed`]: libxml2 then prints its own messages (`Entity: line <n>:
//! parser error : ...` with the line and a caret), which larmorx does not reproduce: it prints one
//! line naming what it found instead, then FreeSurfer's `Document not parsed successfully. `.
//!
//! D13: FS-434 (an element without text, or a comment, processing instruction or CDATA section
//! between the sections, crashes FreeSurfer: both modes print nothing for it), FS-435 (a word longer
//! than a line loses one character at each cut: by default kept, `reproduce` drops it), FS-436 (a
//! comment or processing instruction among the kinds of arguments printed as a section `<NAME>
//! ARGUMENTS`: by default skipped, `reproduce` prints it).

use std::io::Write;

use larmorx_core::defects::{FS_434, FS_435, FS_436};

use crate::globals::Globals;
use crate::run::Environment;

/// A node of the tree libxml2 builds, as fsPrintHelp sees it.
#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    /// An element: its name and its children.
    Element { name: Vec<u8>, children: Vec<Node> },
    /// Character data (adjacent text, references and blanks merged), named `text`.
    Text(Vec<u8>),
    /// A CDATA section (no name).
    CData(Vec<u8>),
    /// A comment (named `comment`, no children).
    Comment,
    /// A processing instruction (named by its target, no children).
    Pi(Vec<u8>),
}

/// The reader's answer.
#[derive(Clone, Debug, PartialEq)]
pub enum Parsed {
    /// A well-formed document: its root element.
    Root(Node),
    /// Not well formed (libxml2's `xmlParseMemory` returns NULL): what was found.
    Malformed(String),
    /// Well formed or not, but holding what the reader does not model.
    Unsupported(String),
}

struct Reader<'a> {
    b: &'a [u8],
    p: usize,
}

type R<T> = Result<T, Parsed>;

fn bad<T>(what: &str) -> R<T> {
    Err(Parsed::Malformed(what.to_owned()))
}

fn unsupported<T>(what: &str) -> R<T> {
    Err(Parsed::Unsupported(what.to_owned()))
}

fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\r')
}

fn name_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

fn name_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.')
}

/// A character XML 1.0 allows (code point `c`).
fn xml_char(c: u32) -> bool {
    matches!(c, 0x9 | 0xA | 0xD | 0x20..=0xD7FF | 0xE000..=0xFFFD | 0x10000..=0x10FFFF)
}

/// The predefined entities' values.
fn predefined(name: &[u8]) -> Option<&'static [u8]> {
    Some(match name {
        b"lt" => b"<",
        b"gt" => b">",
        b"amp" => b"&",
        b"apos" => b"'",
        b"quot" => b"\"",
        _ => return None,
    })
}

impl Reader<'_> {
    fn peek(&self) -> Option<u8> {
        self.b.get(self.p).copied()
    }
    fn starts(&self, s: &[u8]) -> bool {
        self.b[self.p..].starts_with(s)
    }
    fn skip_space(&mut self) -> bool {
        let at = self.p;
        while self.peek().is_some_and(is_space) {
            self.p += 1;
        }
        self.p > at
    }
    fn name(&mut self) -> R<Vec<u8>> {
        match self.peek() {
            Some(c) if name_start(c) => {}
            Some(c) if c == b':' || c >= 0x80 => {
                return unsupported("a name with a colon or outside ASCII");
            }
            _ => return bad("a name expected"),
        }
        let at = self.p;
        while let Some(c) = self.peek() {
            if name_char(c) {
                self.p += 1;
            } else if c == b':' || c >= 0x80 {
                return unsupported("a name with a colon or outside ASCII");
            } else {
                break;
            }
        }
        Ok(self.b[at..self.p].to_vec())
    }
    fn expect(&mut self, s: &[u8], what: &str) -> R<()> {
        if self.starts(s) {
            self.p += s.len();
            Ok(())
        } else {
            bad(what)
        }
    }
    /// A quoted literal (no references): its bytes.
    fn literal(&mut self) -> R<Vec<u8>> {
        let q = match self.peek() {
            Some(q @ (b'"' | b'\'')) => q,
            _ => return bad("a quoted value expected"),
        };
        self.p += 1;
        let at = self.p;
        while let Some(c) = self.peek() {
            if c == q {
                let v = self.b[at..self.p].to_vec();
                self.p += 1;
                return Ok(v);
            }
            self.p += 1;
        }
        bad("a quoted value not closed")
    }
    /// `&...;` at the cursor (after the `&`): the characters it stands for.
    fn reference(&mut self, out: &mut Vec<u8>) -> R<()> {
        if self.peek() == Some(b'#') {
            self.p += 1;
            let hexa = self.peek() == Some(b'x');
            if hexa {
                self.p += 1;
            }
            let at = self.p;
            while self.peek().is_some_and(|c| {
                if hexa {
                    c.is_ascii_hexdigit()
                } else {
                    c.is_ascii_digit()
                }
            }) {
                self.p += 1;
            }
            if self.p == at || self.peek() != Some(b';') {
                return bad("a character reference not closed");
            }
            let digits = std::str::from_utf8(&self.b[at..self.p]).unwrap();
            self.p += 1;
            let v = u32::from_str_radix(digits, if hexa { 16 } else { 10 }).unwrap_or(0);
            match char::from_u32(v).filter(|_| xml_char(v)) {
                Some(c) => {
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                    Ok(())
                }
                None => bad("a character reference to a character XML does not allow"),
            }
        } else {
            let name = self.name()?;
            if self.peek() != Some(b';') {
                return bad("an entity reference not closed");
            }
            self.p += 1;
            match predefined(&name) {
                Some(v) => {
                    out.extend_from_slice(v);
                    Ok(())
                }
                None => unsupported("a reference to an entity that is not predefined"),
            }
        }
    }
    fn comment(&mut self) -> R<()> {
        // after `<!--`
        loop {
            if self.starts(b"--") {
                return if self.starts(b"-->") {
                    self.p += 3;
                    Ok(())
                } else {
                    bad("a double hyphen within a comment")
                };
            }
            if self.peek().is_none() {
                return bad("a comment not terminated");
            }
            self.p += 1;
        }
    }
    fn pi(&mut self) -> R<Vec<u8>> {
        // after `<?`
        let target = self.name()?;
        if target.eq_ignore_ascii_case(b"xml") {
            return bad("an XML declaration not at the start of the document");
        }
        if !self.starts(b"?>") && !self.skip_space() {
            return bad("a processing instruction's target not followed by a space");
        }
        loop {
            if self.starts(b"?>") {
                self.p += 2;
                return Ok(target);
            }
            if self.peek().is_none() {
                return bad("a processing instruction not terminated");
            }
            self.p += 1;
        }
    }
    /// Comments, processing instructions and blanks.
    fn misc(&mut self) -> R<()> {
        loop {
            self.skip_space();
            if self.starts(b"<!--") {
                self.p += 4;
                self.comment()?;
            } else if self.starts(b"<?") {
                self.p += 2;
                self.pi()?;
            } else {
                return Ok(());
            }
        }
    }
    fn doctype(&mut self) -> R<()> {
        // after `<!DOCTYPE`
        if !self.skip_space() {
            return bad("a space expected after DOCTYPE");
        }
        self.name()?;
        self.skip_space();
        if self.starts(b"SYSTEM") {
            self.p += 6;
            self.skip_space();
            self.literal()?;
        } else if self.starts(b"PUBLIC") {
            self.p += 6;
            self.skip_space();
            self.literal()?;
            self.skip_space();
            self.literal()?;
        }
        self.skip_space();
        if self.peek() == Some(b'[') {
            self.p += 1;
            loop {
                self.skip_space();
                if self.peek() == Some(b']') {
                    self.p += 1;
                    break;
                }
                if self.starts(b"<!--") {
                    self.p += 4;
                    self.comment()?;
                } else if self.starts(b"<?") {
                    self.p += 2;
                    self.pi()?;
                } else if self.starts(b"<!ENTITY") {
                    self.p += 8;
                    if !self.skip_space() {
                        return bad("a space expected after ENTITY");
                    }
                    if self.peek() == Some(b'%') {
                        return unsupported("a parameter entity");
                    }
                    self.name()?;
                    if !self.skip_space() {
                        return bad("a space expected after the entity's name");
                    }
                    if self.starts(b"SYSTEM") || self.starts(b"PUBLIC") {
                        return unsupported("an external entity");
                    }
                    let v = self.literal()?;
                    if v.contains(&b'%') || v.contains(&b'<') {
                        return unsupported("an entity value with a parameter reference or a tag");
                    }
                    self.skip_space();
                    self.expect(b">", "an entity declaration not closed")?;
                } else if self.starts(b"<!ELEMENT")
                    || self.starts(b"<!ATTLIST")
                    || self.starts(b"<!NOTATION")
                {
                    // read up to its `>` outside quotes (libxml2 checks their grammar; the
                    // help files' declarations are well formed, and nothing here uses them:
                    // with blanks kept, element declarations change no text)
                    let mut q = None;
                    loop {
                        match (self.peek(), q) {
                            (None, _) => return bad("a declaration not closed"),
                            (Some(c), Some(o)) if c == o => q = None,
                            (Some(c @ (b'"' | b'\'')), None) => q = Some(c),
                            (Some(b'>'), None) => {
                                self.p += 1;
                                break;
                            }
                            (Some(b'%'), None) => {
                                return unsupported("a parameter entity reference");
                            }
                            _ => {}
                        }
                        self.p += 1;
                    }
                } else if self.peek().is_none() {
                    return bad("the internal subset not closed");
                } else {
                    return unsupported(
                        "a declaration other than an entity in the internal subset",
                    );
                }
            }
            self.skip_space();
        }
        self.expect(b">", "the document type not closed")
    }
    /// An element at `<` (the `<` read): the element.
    fn element(&mut self, depth: usize) -> R<Node> {
        if depth > 256 {
            return unsupported("elements nested more than 256 deep");
        }
        let name = self.name()?;
        let mut attrs: Vec<Vec<u8>> = Vec::new();
        loop {
            let spaced = self.skip_space();
            match self.peek() {
                Some(b'>') => {
                    self.p += 1;
                    break;
                }
                Some(b'/') => {
                    self.expect(b"/>", "a start tag not closed")?;
                    return Ok(Node::Element {
                        name,
                        children: Vec::new(),
                    });
                }
                Some(_) if spaced => {
                    let a = self.name()?;
                    if a.starts_with(b"xmlns") {
                        return unsupported("a namespace declaration");
                    }
                    if attrs.contains(&a) {
                        return bad("an attribute given twice");
                    }
                    attrs.push(a);
                    self.skip_space();
                    self.expect(b"=", "an attribute without a value")?;
                    self.skip_space();
                    let q = match self.peek() {
                        Some(q @ (b'"' | b'\'')) => q,
                        _ => return bad("an attribute value not quoted"),
                    };
                    self.p += 1;
                    let mut sink = Vec::new();
                    loop {
                        match self.peek() {
                            None => return bad("an attribute value not closed"),
                            Some(c) if c == q => {
                                self.p += 1;
                                break;
                            }
                            Some(b'<') => return bad("a < in an attribute value"),
                            Some(b'&') => {
                                self.p += 1;
                                self.reference(&mut sink)?;
                            }
                            Some(_) => self.p += 1,
                        }
                    }
                }
                Some(_) => return bad("a start tag not closed"),
                None => return bad("the data ends in a start tag"),
            }
        }
        let mut children = Vec::new();
        let mut text: Vec<u8> = Vec::new();
        let flush = |text: &mut Vec<u8>, children: &mut Vec<Node>| {
            if !text.is_empty() {
                children.push(Node::Text(std::mem::take(text)));
            }
        };
        loop {
            match self.peek() {
                None => return bad("the data ends inside an element"),
                Some(b'<') => {
                    if self.starts(b"</") {
                        self.p += 2;
                        let end = self.name()?;
                        if end != name {
                            return bad("an end tag that does not match its start tag");
                        }
                        self.skip_space();
                        self.expect(b">", "an end tag not closed")?;
                        flush(&mut text, &mut children);
                        return Ok(Node::Element { name, children });
                    } else if self.starts(b"<!--") {
                        self.p += 4;
                        self.comment()?;
                        flush(&mut text, &mut children);
                        children.push(Node::Comment);
                    } else if self.starts(b"<![CDATA[") {
                        self.p += 9;
                        let at = self.p;
                        while !self.starts(b"]]>") {
                            if self.peek().is_none() {
                                return bad("a CDATA section not terminated");
                            }
                            self.p += 1;
                        }
                        flush(&mut text, &mut children);
                        children.push(Node::CData(self.b[at..self.p].to_vec()));
                        self.p += 3;
                    } else if self.starts(b"<?") {
                        self.p += 2;
                        let t = self.pi()?;
                        flush(&mut text, &mut children);
                        children.push(Node::Pi(t));
                    } else if self.starts(b"<!") {
                        return bad("a markup declaration in content");
                    } else {
                        self.p += 1;
                        flush(&mut text, &mut children);
                        let e = self.element(depth + 1)?;
                        children.push(e);
                    }
                }
                Some(b'&') => {
                    self.p += 1;
                    self.reference(&mut text)?;
                }
                Some(_) => {
                    if self.starts(b"]]>") {
                        return bad("the sequence ]]> in content");
                    }
                    text.push(self.b[self.p]);
                    self.p += 1;
                }
            }
        }
    }
}

/// The text libxml2 reads: the byte-order mark skipped, the declared encoding (UTF-8,
/// ISO-8859-1, ASCII) turned into UTF-8 and checked, line ends made `\n`.
fn decode(text: &[u8]) -> R<Vec<u8>> {
    let mut b = text;
    if b.starts_with(&[0xEF, 0xBB, 0xBF]) {
        b = &b[3..];
    }
    if b.starts_with(&[0xFE, 0xFF])
        || b.starts_with(&[0xFF, 0xFE])
        || b.starts_with(&[0, b'<'])
        || b.starts_with(&[b'<', 0])
    {
        return unsupported("a document in UTF-16 or UCS-4");
    }
    // the declared encoding
    let mut latin1 = false;
    if b.starts_with(b"<?xml") && b.get(5).is_some_and(|&c| is_space(c)) {
        let end = b.windows(2).position(|w| w == b"?>").unwrap_or(b.len());
        let decl = &b[..end];
        if let Some(e) = decl.windows(8).position(|w| w == b"encoding") {
            let rest = &decl[e + 8..];
            let q = rest.iter().position(|&c| c == b'"' || c == b'\'');
            if let Some(q) = q {
                let qc = rest[q];
                let v = &rest[q + 1..];
                let n = v.iter().position(|&c| c == qc).unwrap_or(v.len());
                let enc = v[..n].to_ascii_uppercase();
                match enc.as_slice() {
                    b"UTF-8" | b"UTF8" => {}
                    b"ISO-8859-1" | b"ISO-LATIN-1" | b"ISO LATIN 1" => latin1 = true,
                    b"ASCII" | b"US-ASCII" => {
                        if b.iter().any(|&c| c >= 0x80) {
                            return bad("a byte outside ASCII in an ASCII document");
                        }
                    }
                    _ => return unsupported("an encoding other than UTF-8, ISO-8859-1 or ASCII"),
                }
            }
        }
    }
    let mut v: Vec<u8> = Vec::with_capacity(b.len());
    if latin1 {
        for &c in b {
            if c < 0x80 {
                v.push(c);
            } else {
                v.push(0xC0 | (c >> 6));
                v.push(0x80 | (c & 0x3F));
            }
        }
    } else {
        v.extend_from_slice(b);
    }
    let s = match std::str::from_utf8(&v) {
        Ok(s) => s,
        Err(_) => return bad("bytes that are not UTF-8"),
    };
    if s.chars().any(|c| !xml_char(c as u32)) {
        return bad("a character XML does not allow");
    }
    let mut out = Vec::with_capacity(v.len());
    let mut i = 0;
    while i < v.len() {
        if v[i] == b'\r' {
            out.push(b'\n');
            if v.get(i + 1) == Some(&b'\n') {
                i += 1;
            }
        } else {
            out.push(v[i]);
        }
        i += 1;
    }
    Ok(out)
}

/// `xmlParseMemory(text, size)` as fsPrintHelp uses it: the root element of a well-formed document.
pub fn parse(text: &[u8]) -> Parsed {
    let b = match decode(text) {
        Ok(b) => b,
        Err(e) => return e,
    };
    let mut r = Reader { b: &b, p: 0 };
    let run = |r: &mut Reader| -> R<Node> {
        if r.starts(b"<?xml") && r.b.get(5).is_some_and(|&c| is_space(c)) {
            r.p += 5;
            r.skip_space();
            r.expect(b"version", "an XML declaration without its version")?;
            r.skip_space();
            r.expect(b"=", "an XML declaration without its version")?;
            r.skip_space();
            let v = r.literal()?;
            if !v.starts_with(b"1.") {
                return unsupported("an XML version other than 1.x");
            }
            loop {
                let spaced = r.skip_space();
                if r.starts(b"?>") {
                    r.p += 2;
                    break;
                }
                if !spaced {
                    return bad("an XML declaration not closed");
                }
                let n = r.name()?;
                if n != b"encoding" && n != b"standalone" {
                    return bad("an unknown attribute of the XML declaration");
                }
                r.skip_space();
                r.expect(b"=", "an XML declaration's attribute without a value")?;
                r.skip_space();
                r.literal()?;
            }
        }
        r.misc()?;
        if r.starts(b"<!DOCTYPE") {
            r.p += 9;
            r.doctype()?;
            r.misc()?;
        }
        if r.peek() != Some(b'<') || r.starts(b"<!") || r.starts(b"<?") {
            return bad(if r.peek().is_none() {
                "the document is empty"
            } else {
                "a start tag expected"
            });
        }
        r.p += 1;
        let root = r.element(0)?;
        r.misc()?;
        if r.peek().is_some() {
            return bad("extra content at the end of the document");
        }
        Ok(root)
    };
    match run(&mut r) {
        Ok(root) => Parsed::Root(root),
        Err(e) => e,
    }
}

/// The node's name as libxml2 gives it (`None`: a CDATA section's, none).
fn node_name(n: &Node) -> Option<&[u8]> {
    match n {
        Node::Element { name, .. } => Some(name),
        Node::Text(_) => Some(b"text"),
        Node::Comment => Some(b"comment"),
        Node::Pi(t) => Some(t),
        Node::CData(_) => None,
    }
}

/// `tagNameIs(c, cur)`: the name equals `c` without regard to ASCII case.
fn tag_name_is(c: &[u8], n: &Node) -> bool {
    node_name(n).is_some_and(|m| m.eq_ignore_ascii_case(c))
}

fn children(n: &Node) -> &[Node] {
    match n {
        Node::Element { children, .. } => children,
        _ => &[],
    }
}

/// A comment, a processing instruction or a CDATA section where fsPrintHelp expects a section or
/// an argument: FreeSurfer crashes there (FS-434); larmorx skips it.
fn structural_stray(g: &mut Globals, n: &Node) -> bool {
    if matches!(n, Node::Comment | Node::Pi(_) | Node::CData(_)) {
        g.notices.add(
            &FS_434,
            "the help text holds a comment, processing instruction or CDATA section between its sections (FreeSurfer crashes): skipped",
        );
        return true;
    }
    false
}

/// `printName(cur)`: two line ends and the name in capitals, its first `_` and its first `-` made
/// spaces.
fn print_name(out: &mut dyn Write, n: &Node) {
    let mut s: Vec<u8> = node_name(n).unwrap_or_default().to_ascii_uppercase();
    if let Some(p) = s.iter().position(|&c| c == b'_') {
        s[p] = b' ';
    }
    if let Some(p) = s.iter().position(|&c| c == b'-') {
        s[p] = b' ';
    }
    let _ = out.write_all(b"\n\n");
    let _ = out.write_all(&s);
}

/// `wrdLen(c)`: the bytes before the first space, `_` or `/` (or the end).
fn wrd_len(c: &[u8]) -> usize {
    c.iter()
        .position(|&x| x == b' ' || x == b'_' || x == b'/')
        .unwrap_or(c.len())
}

/// `FSPRINT_MAX_CHARS`.
const MAX_CHARS: usize = 78;

/// `printContents(doc, cur)`: the element's text (`xmlNodeListGetString` of its children: their
/// text and CDATA sections) in lines of at most 70 characters (62 in an `explanation`), each
/// after a line end and a tab (two in an explanation), one leading space dropped, words kept
/// whole when they fit, a text line end starting a new line. FS-434: an element without text
/// crashes FreeSurfer; larmorx prints nothing. FS-435: a word longer than a line is cut at the
/// line's end and its next character dropped; by default kept (`reproduce` drops it).
fn print_contents(g: &mut Globals, out: &mut dyn Write, n: &Node) {
    let kids = children(n);
    let mut contents: Option<Vec<u8>> = None;
    for k in kids {
        if let Node::Text(t) | Node::CData(t) = k {
            contents.get_or_insert_with(Vec::new).extend_from_slice(t);
        }
    }
    let Some(contents) = contents else {
        g.notices.add(
            &FS_434,
            "the help text holds an element without text (FreeSurfer crashes): nothing printed for it",
        );
        return;
    };
    // strlen: the text up to its first NUL (none in a parsed document)
    let len = contents
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(contents.len());
    let c = &contents[..len];
    let explanation = tag_name_is(b"explanation", n);
    let mut line = Vec::new();
    let mut i = 0;
    while i < len {
        let tab_num = if explanation { 2 } else { 1 };
        line.extend_from_slice(if explanation { b"\n\t\t" } else { b"\n\t" });
        if c[i] == b' ' {
            i += 1;
        }
        let width = MAX_CHARS - tab_num * 8;
        let mut j = i;
        while j < width + i
            && j < len
            && (wrd_len(&c[j..]) > width || wrd_len(&c[j..]) + j <= width + i)
        {
            if c[j] == b'\n' {
                j += 1;
                break;
            } else if wrd_len(&c[j..]) > width {
                while j < width + i {
                    line.push(c[j]);
                    j += 1;
                }
                // the loop's increment then passes one more character
                if g.notices.reproduce() {
                    j += 1;
                }
                g.notices.add(
                    &FS_435,
                    "a help text word longer than a line: FreeSurfer drops its character after each cut",
                );
                if !g.notices.reproduce() {
                    break;
                }
                continue;
            } else {
                line.push(c[j]);
            }
            j += 1;
        }
        i = j;
    }
    let _ = out.write_all(&line);
}

/// `outputHelpDoc(doc)`: the help text of a parsed document (its root `help`, in any case):
/// `\t\t\t\tHelp`, then each section's name and text; an `arguments` section prints each kind of
/// argument (`<NAME> ARGUMENTS`) and its intro and arguments, with a line end before each intro or
/// argument but the first; `example` sections are numbered; an `outputs` section prints its
/// intro and outputs the same way; sections named `text` are skipped. Three line ends; 0. A root of
/// another name: `Document id of the wrong type!  The root node is not help.` on standard error,
/// -1.
pub fn output_help_doc(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    root: &Node,
) -> i32 {
    if !tag_name_is(b"help", root) {
        let _ = err.write_all(b"Document id of the wrong type!  The root node is not help.\n");
        return -1;
    }
    let _ = out.write_all(b"\t\t\t\tHelp");
    let mut example_num = 1;
    for cur in children(root) {
        if tag_name_is(b"arguments", cur) {
            for at in children(cur) {
                if tag_name_is(b"text", at) {
                    continue;
                }
                if matches!(at, Node::Comment | Node::Pi(_)) {
                    // FS-436: FreeSurfer prints it as a kind of argument without arguments
                    g.notices.add(
                        &FS_436,
                        "a comment or processing instruction among the help text's kinds of arguments: FreeSurfer prints it as a section `<NAME> ARGUMENTS`",
                    );
                    if g.notices.reproduce() {
                        print_name(out, at);
                        let _ = out.write_all(b" ARGUMENTS");
                    }
                    continue;
                }
                if structural_stray(g, at) {
                    continue;
                }
                print_name(out, at);
                let _ = out.write_all(b" ARGUMENTS");
                let mut first = true;
                for ae in children(at) {
                    if tag_name_is(b"text", ae) || structural_stray(g, ae) {
                        continue;
                    }
                    if tag_name_is(b"intro", ae) || tag_name_is(b"argument", ae) {
                        if first {
                            first = false;
                        } else {
                            let _ = out.write_all(b"\n");
                        }
                    }
                    print_contents(g, out, ae);
                }
            }
        } else if !tag_name_is(b"text", cur) {
            if structural_stray(g, cur) {
                continue;
            }
            print_name(out, cur);
            if tag_name_is(b"example", cur) {
                let _ = write!(out, " {example_num}");
                example_num += 1;
            }
            if tag_name_is(b"outputs", cur) {
                let mut first = true;
                for oe in children(cur) {
                    if tag_name_is(b"text", oe) || structural_stray(g, oe) {
                        continue;
                    }
                    if tag_name_is(b"intro", oe) || tag_name_is(b"output", oe) {
                        if first {
                            first = false;
                        } else {
                            let _ = out.write_all(b"\n");
                        }
                    }
                    print_contents(g, out, oe);
                }
            } else {
                print_contents(g, out, cur);
            }
        }
    }
    let _ = out.write_all(b"\n\n\n");
    0
}

/// The parse of a text and its rendering: -1 after `Document not parsed successfully. ` when it
/// does not parse (an empty text without a message, as `xmlParseMemory`; otherwise larmorx's line
/// naming the problem, where libxml2 prints its own messages).
fn render(g: &mut Globals, out: &mut dyn Write, err: &mut dyn Write, text: &[u8]) -> i32 {
    if text.is_empty() {
        let _ = err.write_all(b"Document not parsed successfully. \n");
        return -1;
    }
    match parse(text) {
        Parsed::Root(root) => output_help_doc(g, out, err, &root),
        Parsed::Malformed(what) => {
            let _ = writeln!(
                err,
                "larmorx-freesurfer: the help text is not well-formed XML ({what}); libxml2's messages are not reproduced"
            );
            let _ = err.write_all(b"Document not parsed successfully. \n");
            -1
        }
        Parsed::Unsupported(what) => {
            let _ = writeln!(
                err,
                "larmorx-freesurfer: the help text holds XML the port does not read ({what})"
            );
            let _ = err.write_all(b"Document not parsed successfully. \n");
            -1
        }
    }
}

/// `outputHelpXml(text, size)`: the help text of a program's XML (`None`: `No xml string
/// passed.`, -1).
pub fn output_help_xml(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    text: Option<&[u8]>,
) -> i32 {
    match text {
        None => {
            let _ = err.write_all(b"No xml string passed.\n");
            -1
        }
        Some(t) => render(g, out, err, t),
    }
}

/// `outputHelp(name)`: the help text of the file `name`, or, when no such file opens, of
/// `$FREESURFER_HOME/docs/xml/<name>.help.xml` (no `FREESURFER_HOME`: -1, nothing printed).
/// `None`: `No filename passed.`, -1. A file that does not open or parse: `Document not parsed
/// successfully. `, -1.
pub fn output_help(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&std::path::Path>,
    name: Option<&str>,
) -> i32 {
    let Some(name) = name else {
        let _ = err.write_all(b"No filename passed.\n");
        return -1;
    };
    let resolve = |n: &str| crate::utils::resolve(dir, n);
    let path = if std::fs::File::open(resolve(name)).is_ok() {
        resolve(name)
    } else {
        let Some(home) = env.get("FREESURFER_HOME") else {
            return -1;
        };
        resolve(&format!(
            "{}/docs/xml/{name}.help.xml",
            home.to_string_lossy()
        ))
    };
    match std::fs::read(&path) {
        Ok(b) if !b.is_empty() => render(g, out, err, &b),
        // xmlParseFile of a file that does not open or is empty: libxml2's message, then this
        _ => {
            let _ = err.write_all(b"Document not parsed successfully. \n");
            -1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_text(text: &[u8], reproduce: bool) -> (i32, String, String) {
        use larmorx_core::defects::UpstreamDefects;
        let mut g = Globals::new(if reproduce {
            UpstreamDefects::Reproduce
        } else {
            UpstreamDefects::Fix
        });
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let r = output_help_xml(&mut g, &mut o, &mut e, Some(text));
        (
            r,
            String::from_utf8(o).unwrap(),
            String::from_utf8(e).unwrap(),
        )
    }

    #[test]
    fn sections_arguments_and_wrapping() {
        let t = b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?>\n<!DOCTYPE help [\n<!ENTITY lt \"#38;#60;\">\n<!ENTITY gt \"&#62;\">\n<!ENTITY amp \"&#38;#38;\">\n]>\n<help>\n<name>prog</name>\n<synopsis>prog -i &lt;in&gt; &amp; more</synopsis>\n<arguments><required-flagged><argument>-i in</argument><explanation>the input</explanation></required-flagged></arguments>\n<example>prog -i x</example><example>two</example>\n</help>\n";
        let (r, o, e) = render_text(t, false);
        assert_eq!(r, 0);
        assert_eq!(e, "");
        assert_eq!(
            o,
            "\t\t\t\tHelp\n\nNAME\n\tprog\n\nSYNOPSIS\n\tprog -i <in> & more\n\nREQUIRED FLAGGED ARGUMENTS\n\t-i in\n\t\tthe input\n\nEXAMPLE 1\n\tprog -i x\n\nEXAMPLE 2\n\ttwo\n\n\n"
        );
    }

    #[test]
    fn long_words_and_errors() {
        let word = "a".repeat(75);
        let t = format!("<help><name>{word}</name></help>");
        let (_, fixed, _) = render_text(t.as_bytes(), false);
        let (_, repro, _) = render_text(t.as_bytes(), true);
        assert_eq!(
            fixed,
            format!(
                "\t\t\t\tHelp\n\nNAME\n\t{}\n\t{}\n\n\n",
                "a".repeat(70),
                "a".repeat(5)
            )
        );
        assert_eq!(
            repro,
            format!(
                "\t\t\t\tHelp\n\nNAME\n\t{}\n\t{}\n\n\n",
                "a".repeat(70),
                "a".repeat(4)
            )
        );
        let (r, _, e) = render_text(b"<other/>", false);
        assert_eq!(
            (r, e.as_str()),
            (
                -1,
                "Document id of the wrong type!  The root node is not help.\n"
            )
        );
        let (r, _, e) = render_text(b"", false);
        assert_eq!(
            (r, e.as_str()),
            (-1, "Document not parsed successfully. \n")
        );
        assert!(matches!(parse(b"<help><b></help>"), Parsed::Malformed(_)));
        assert!(matches!(parse(b"<a:help/>"), Parsed::Unsupported(_)));
    }
}
