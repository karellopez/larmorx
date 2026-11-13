// SPDX-License-Identifier: Apache-2.0 AND MIT
// Translated from expat 2.0.1's xmltok.c, xmltok_impl.c, asciitab.h, iasciitab.h, latin1tab.h and
// utf8tab.h (Copyright (c) 1998, 1999, 2000 Thai Open Source Software Center Ltd and Clark
// Cooper; Copyright (c) 2001, 2002, 2003, 2004, 2005, 2006 Expat maintainers; MIT, PROVENANCE.md):
// the built-in encodings (UTF-8, ISO-8859-1, US-ASCII, UTF-16 in both byte orders and the
// internal UTF-8 of entity texts), their byte types, name-character checks and conversions to
// UTF-8, and the tokenizers, for a parser without namespace processing.

use super::nametab::{NAME_PAGES, NAMING_BITMAP, NMSTRT_PAGES};

// ---------------------------------------------------------------- byte types (xmltok_impl.h)

pub(super) const BT_NONXML: u8 = 0;
pub(super) const BT_MALFORM: u8 = 1;
pub(super) const BT_LT: u8 = 2;
pub(super) const BT_AMP: u8 = 3;
pub(super) const BT_RSQB: u8 = 4;
pub(super) const BT_LEAD2: u8 = 5;
pub(super) const BT_LEAD3: u8 = 6;
pub(super) const BT_LEAD4: u8 = 7;
pub(super) const BT_TRAIL: u8 = 8;
pub(super) const BT_CR: u8 = 9;
pub(super) const BT_LF: u8 = 10;
pub(super) const BT_GT: u8 = 11;
pub(super) const BT_QUOT: u8 = 12;
pub(super) const BT_APOS: u8 = 13;
pub(super) const BT_EQUALS: u8 = 14;
pub(super) const BT_QUEST: u8 = 15;
pub(super) const BT_EXCL: u8 = 16;
pub(super) const BT_SOL: u8 = 17;
pub(super) const BT_SEMI: u8 = 18;
pub(super) const BT_NUM: u8 = 19;
pub(super) const BT_LSQB: u8 = 20;
pub(super) const BT_S: u8 = 21;
pub(super) const BT_NMSTRT: u8 = 22;
pub(super) const BT_HEX: u8 = 24;
pub(super) const BT_DIGIT: u8 = 25;
pub(super) const BT_NAME: u8 = 26;
pub(super) const BT_MINUS: u8 = 27;
pub(super) const BT_OTHER: u8 = 28;
pub(super) const BT_NONASCII: u8 = 29;
pub(super) const BT_PERCNT: u8 = 30;
pub(super) const BT_LPAR: u8 = 31;
pub(super) const BT_RPAR: u8 = 32;
pub(super) const BT_AST: u8 = 33;
pub(super) const BT_PLUS: u8 = 34;
pub(super) const BT_COMMA: u8 = 35;
pub(super) const BT_VERBAR: u8 = 36;

/// asciitab.h for a parser without namespaces (':' a name start character); `cr` is 0x0D's type
/// (BT_S in iasciitab.h).
const fn ascii_tab(cr: u8) -> [u8; 128] {
    let mut t = [BT_NONXML; 128];
    t[0x09] = BT_S;
    t[0x0A] = BT_LF;
    t[0x0D] = cr;
    t[0x20] = BT_S;
    t[0x21] = BT_EXCL;
    t[0x22] = BT_QUOT;
    t[0x23] = BT_NUM;
    t[0x24] = BT_OTHER;
    t[0x25] = BT_PERCNT;
    t[0x26] = BT_AMP;
    t[0x27] = BT_APOS;
    t[0x28] = BT_LPAR;
    t[0x29] = BT_RPAR;
    t[0x2A] = BT_AST;
    t[0x2B] = BT_PLUS;
    t[0x2C] = BT_COMMA;
    t[0x2D] = BT_MINUS;
    t[0x2E] = BT_NAME;
    t[0x2F] = BT_SOL;
    let mut c = 0x30;
    while c <= 0x39 {
        t[c] = BT_DIGIT;
        c += 1;
    }
    t[0x3A] = BT_NMSTRT;
    t[0x3B] = BT_SEMI;
    t[0x3C] = BT_LT;
    t[0x3D] = BT_EQUALS;
    t[0x3E] = BT_GT;
    t[0x3F] = BT_QUEST;
    t[0x40] = BT_OTHER;
    let mut c = 0x41;
    while c <= 0x5A {
        t[c] = if c <= 0x46 { BT_HEX } else { BT_NMSTRT };
        t[c + 0x20] = t[c];
        c += 1;
    }
    t[0x5B] = BT_LSQB;
    t[0x5C] = BT_OTHER;
    t[0x5D] = BT_RSQB;
    t[0x5E] = BT_OTHER;
    t[0x5F] = BT_NMSTRT;
    t[0x60] = BT_OTHER;
    t[0x7B] = BT_OTHER;
    t[0x7C] = BT_VERBAR;
    t[0x7D] = BT_OTHER;
    t[0x7E] = BT_OTHER;
    t[0x7F] = BT_OTHER;
    t
}

/// latin1tab.h (0x80..0xFF).
const fn latin1_high() -> [u8; 128] {
    let mut t = [BT_OTHER; 128];
    t[0xAA - 0x80] = BT_NMSTRT;
    t[0xB5 - 0x80] = BT_NMSTRT;
    t[0xB7 - 0x80] = BT_NAME;
    t[0xBA - 0x80] = BT_NMSTRT;
    let mut c = 0xC0;
    while c <= 0xFF {
        if c != 0xD7 && c != 0xF7 {
            t[c - 0x80] = BT_NMSTRT;
        }
        c += 1;
    }
    t
}

/// utf8tab.h (0x80..0xFF).
const fn utf8_high() -> [u8; 128] {
    let mut t = [BT_TRAIL; 128];
    let mut c = 0xC0;
    while c <= 0xFF {
        t[c - 0x80] = if c <= 0xDF {
            BT_LEAD2
        } else if c <= 0xEF {
            BT_LEAD3
        } else if c <= 0xF4 {
            BT_LEAD4
        } else if c <= 0xFD {
            BT_NONXML
        } else {
            BT_MALFORM
        };
        c += 1;
    }
    t
}

const fn join(lo: [u8; 128], hi: [u8; 128]) -> [u8; 256] {
    let mut t = [0u8; 256];
    let mut i = 0;
    while i < 128 {
        t[i] = lo[i];
        t[i + 128] = hi[i];
        i += 1;
    }
    t
}

static T_UTF8: [u8; 256] = join(ascii_tab(BT_CR), utf8_high());
static T_INTERNAL: [u8; 256] = join(ascii_tab(BT_S), utf8_high());
static T_LATIN1: [u8; 256] = join(ascii_tab(BT_CR), latin1_high());
static T_ASCII: [u8; 256] = join(ascii_tab(BT_CR), [BT_NONXML; 128]);

// ---------------------------------------------------------------- tokens (xmltok.h)

pub(super) const TOK_TRAILING_RSQB: i32 = -5;
pub(super) const TOK_NONE: i32 = -4;
pub(super) const TOK_TRAILING_CR: i32 = -3;
pub(super) const TOK_PARTIAL_CHAR: i32 = -2;
pub(super) const TOK_PARTIAL: i32 = -1;
pub(super) const TOK_INVALID: i32 = 0;
pub(super) const TOK_START_TAG_WITH_ATTS: i32 = 1;
pub(super) const TOK_START_TAG_NO_ATTS: i32 = 2;
pub(super) const TOK_EMPTY_ELEMENT_WITH_ATTS: i32 = 3;
pub(super) const TOK_EMPTY_ELEMENT_NO_ATTS: i32 = 4;
pub(super) const TOK_END_TAG: i32 = 5;
pub(super) const TOK_DATA_CHARS: i32 = 6;
pub(super) const TOK_DATA_NEWLINE: i32 = 7;
pub(super) const TOK_CDATA_SECT_OPEN: i32 = 8;
pub(super) const TOK_ENTITY_REF: i32 = 9;
pub(super) const TOK_CHAR_REF: i32 = 10;
pub(super) const TOK_PI: i32 = 11;
pub(super) const TOK_XML_DECL: i32 = 12;
pub(super) const TOK_COMMENT: i32 = 13;
pub(super) const TOK_BOM: i32 = 14;
pub(super) const TOK_PROLOG_S: i32 = 15;
pub(super) const TOK_DECL_OPEN: i32 = 16;
pub(super) const TOK_DECL_CLOSE: i32 = 17;
pub(super) const TOK_NAME: i32 = 18;
pub(super) const TOK_NMTOKEN: i32 = 19;
pub(super) const TOK_POUND_NAME: i32 = 20;
pub(super) const TOK_OR: i32 = 21;
pub(super) const TOK_PERCENT: i32 = 22;
pub(super) const TOK_OPEN_PAREN: i32 = 23;
pub(super) const TOK_CLOSE_PAREN: i32 = 24;
pub(super) const TOK_OPEN_BRACKET: i32 = 25;
pub(super) const TOK_CLOSE_BRACKET: i32 = 26;
pub(super) const TOK_LITERAL: i32 = 27;
pub(super) const TOK_PARAM_ENTITY_REF: i32 = 28;
pub(super) const TOK_INSTANCE_START: i32 = 29;
pub(super) const TOK_NAME_QUESTION: i32 = 30;
pub(super) const TOK_NAME_ASTERISK: i32 = 31;
pub(super) const TOK_NAME_PLUS: i32 = 32;
pub(super) const TOK_COND_SECT_OPEN: i32 = 33;
pub(super) const TOK_COND_SECT_CLOSE: i32 = 34;
pub(super) const TOK_CLOSE_PAREN_QUESTION: i32 = 35;
pub(super) const TOK_CLOSE_PAREN_ASTERISK: i32 = 36;
pub(super) const TOK_CLOSE_PAREN_PLUS: i32 = 37;
pub(super) const TOK_COMMA: i32 = 38;
pub(super) const TOK_ATTRIBUTE_VALUE_S: i32 = 39;
pub(super) const TOK_CDATA_SECT_CLOSE: i32 = 40;

// ---------------------------------------------------------------- encodings

/// expat's built-in encodings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Enc {
    Utf8,
    /// The internal encoding of stored entity texts (iasciitab.h: CR is white space).
    Internal,
    Latin1,
    Ascii,
    Little2,
    Big2,
}

/// The outcome of a character case of the scanners (INVALID_CASES, CHECK_NAME_CASES and kin).
enum Step {
    Advanced,
    Return(i32),
    Other,
}

fn naming2(pages: &[u8; 256], p: &[u8]) -> bool {
    let (b0, b1) = (p[0] as usize, p[1] as usize);
    let i = ((pages[(b0 >> 2) & 7] as usize) << 3) + ((b0 & 3) << 1) + ((b1 >> 5) & 1);
    NAMING_BITMAP[i] & (1u32 << (b1 & 0x1F)) != 0
}

fn naming3(pages: &[u8; 256], p: &[u8]) -> bool {
    let (b0, b1, b2) = (p[0] as usize, p[1] as usize, p[2] as usize);
    let i = ((pages[((b0 & 0xF) << 4) + ((b1 >> 2) & 0xF)] as usize) << 3)
        + ((b1 & 3) << 1)
        + ((b2 >> 5) & 1);
    NAMING_BITMAP[i] & (1u32 << (b2 & 0x1F)) != 0
}

fn ucs2_naming(pages: &[u8; 256], hi: u8, lo: u8) -> bool {
    let i = ((pages[hi as usize] as usize) << 3) + ((lo as usize) >> 5);
    NAMING_BITMAP[i] & (1u32 << (lo & 0x1F)) != 0
}

fn utf8_invalid(p: &[u8], n: usize) -> bool {
    match n {
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

/// The second byte of a UTF-16 unit (0 past the end of the buffer: expat reads past it).
fn at1(b: &[u8], p: usize) -> u8 {
    b.get(p + 1).copied().unwrap_or(0)
}

/// `unicode_byte_type` of a UTF-16 unit whose high byte is not 0.
fn unicode_bt(hi: u8, lo: u8) -> u8 {
    match hi {
        0xD8..=0xDB => BT_LEAD4,
        0xDC..=0xDF => BT_TRAIL,
        0xFF if lo == 0xFF || lo == 0xFE => BT_NONXML,
        _ => BT_NONASCII,
    }
}

fn lead(bt: u8) -> usize {
    match bt {
        BT_LEAD2 => 2,
        BT_LEAD3 => 3,
        _ => 4,
    }
}

impl Enc {
    pub(super) fn mbpc(self) -> usize {
        match self {
            Enc::Little2 | Enc::Big2 => 2,
            _ => 1,
        }
    }

    /// `isUtf8`: characters reach the handlers unconverted (`!MUST_CONVERT`).
    pub(super) fn is_utf8(self) -> bool {
        matches!(self, Enc::Utf8 | Enc::Internal | Enc::Ascii)
    }

    fn table(self) -> &'static [u8; 256] {
        match self {
            Enc::Utf8 => &T_UTF8,
            Enc::Internal => &T_INTERNAL,
            Enc::Ascii => &T_ASCII,
            _ => &T_LATIN1,
        }
    }

    /// `BYTE_TYPE`.
    pub(super) fn bt(self, b: &[u8], p: usize) -> u8 {
        match self {
            Enc::Little2 => {
                if at1(b, p) == 0 {
                    T_LATIN1[b[p] as usize]
                } else {
                    unicode_bt(at1(b, p), b[p])
                }
            }
            Enc::Big2 => {
                if b[p] == 0 {
                    T_LATIN1[at1(b, p) as usize]
                } else {
                    unicode_bt(b[p], at1(b, p))
                }
            }
            _ => self.table()[b[p] as usize],
        }
    }

    /// `BYTE_TO_ASCII`: the character as a C `char` (signed), -1 for a UTF-16 unit above 0xFF.
    pub(super) fn byte_to_ascii(self, b: &[u8], p: usize) -> i32 {
        match self {
            Enc::Little2 => {
                if at1(b, p) == 0 {
                    b[p] as i8 as i32
                } else {
                    -1
                }
            }
            Enc::Big2 => {
                if b[p] == 0 {
                    at1(b, p) as i8 as i32
                } else {
                    -1
                }
            }
            _ => b[p] as i8 as i32,
        }
    }

    /// `CHAR_MATCHES` with an ASCII character.
    pub(super) fn char_matches(self, b: &[u8], p: usize, c: u8) -> bool {
        match self {
            Enc::Little2 => at1(b, p) == 0 && b[p] == c,
            Enc::Big2 => b[p] == 0 && at1(b, p) == c,
            _ => b[p] == c,
        }
    }

    fn is_name(self, b: &[u8], p: usize, n: usize) -> bool {
        match (self, n) {
            (Enc::Utf8 | Enc::Internal, 2) => naming2(&NAME_PAGES, &b[p..]),
            (Enc::Utf8 | Enc::Internal, 3) => naming3(&NAME_PAGES, &b[p..]),
            _ => false,
        }
    }

    fn is_nmstrt(self, b: &[u8], p: usize, n: usize) -> bool {
        match (self, n) {
            (Enc::Utf8 | Enc::Internal, 2) => naming2(&NMSTRT_PAGES, &b[p..]),
            (Enc::Utf8 | Enc::Internal, 3) => naming3(&NMSTRT_PAGES, &b[p..]),
            _ => false,
        }
    }

    fn is_invalid(self, b: &[u8], p: usize, n: usize) -> bool {
        match self {
            Enc::Utf8 | Enc::Internal => utf8_invalid(&b[p..p + n], n),
            _ => false,
        }
    }

    fn is_name_min(self, b: &[u8], p: usize) -> bool {
        match self {
            Enc::Little2 => ucs2_naming(&NAME_PAGES, at1(b, p), b[p]),
            Enc::Big2 => ucs2_naming(&NAME_PAGES, b[p], at1(b, p)),
            _ => false,
        }
    }

    fn is_nmstrt_min(self, b: &[u8], p: usize) -> bool {
        match self {
            Enc::Little2 => ucs2_naming(&NMSTRT_PAGES, at1(b, p), b[p]),
            Enc::Big2 => ucs2_naming(&NMSTRT_PAGES, b[p], at1(b, p)),
            _ => false,
        }
    }

    // ------------------------------------------------------------ the scanners' cases

    /// INVALID_CASES.
    fn invalid_case(self, b: &[u8], p: &mut usize, end: usize, next: &mut usize) -> Step {
        match self.bt(b, *p) {
            t @ (BT_LEAD2 | BT_LEAD3 | BT_LEAD4) => {
                let n = lead(t);
                if end - *p < n {
                    return Step::Return(TOK_PARTIAL_CHAR);
                }
                if self.is_invalid(b, *p, n) {
                    *next = *p;
                    return Step::Return(TOK_INVALID);
                }
                *p += n;
                Step::Advanced
            }
            BT_NONXML | BT_MALFORM | BT_TRAIL => {
                *next = *p;
                Step::Return(TOK_INVALID)
            }
            _ => Step::Other,
        }
    }

    /// CHECK_NAME_CASES.
    fn name_case(self, b: &[u8], p: &mut usize, end: usize, next: &mut usize) -> Step {
        match self.bt(b, *p) {
            BT_NONASCII => {
                if !self.is_name_min(b, *p) {
                    *next = *p;
                    return Step::Return(TOK_INVALID);
                }
                *p += self.mbpc();
                Step::Advanced
            }
            BT_NMSTRT | BT_HEX | BT_DIGIT | BT_NAME | BT_MINUS => {
                *p += self.mbpc();
                Step::Advanced
            }
            t @ (BT_LEAD2 | BT_LEAD3 | BT_LEAD4) => {
                let n = lead(t);
                if end - *p < n {
                    return Step::Return(TOK_PARTIAL_CHAR);
                }
                if !self.is_name(b, *p, n) {
                    *next = *p;
                    return Step::Return(TOK_INVALID);
                }
                *p += n;
                Step::Advanced
            }
            _ => Step::Other,
        }
    }

    /// CHECK_NMSTRT_CASES.
    fn nmstrt_case(self, b: &[u8], p: &mut usize, end: usize, next: &mut usize) -> Step {
        match self.bt(b, *p) {
            BT_NONASCII => {
                if !self.is_nmstrt_min(b, *p) {
                    *next = *p;
                    return Step::Return(TOK_INVALID);
                }
                *p += self.mbpc();
                Step::Advanced
            }
            BT_NMSTRT | BT_HEX => {
                *p += self.mbpc();
                Step::Advanced
            }
            t @ (BT_LEAD2 | BT_LEAD3 | BT_LEAD4) => {
                let n = lead(t);
                if end - *p < n {
                    return Step::Return(TOK_PARTIAL_CHAR);
                }
                if !self.is_nmstrt(b, *p, n) {
                    *next = *p;
                    return Step::Return(TOK_INVALID);
                }
                *p += n;
                Step::Advanced
            }
            _ => Step::Other,
        }
    }

    /// The end of a scan cut to whole units (UTF-16): `None` when not one unit remains.
    fn whole_units(self, p: usize, end: usize) -> Option<usize> {
        let m = self.mbpc();
        let n = end - p;
        if m > 1 && n & (m - 1) != 0 {
            let n = n & !(m - 1);
            if n == 0 {
                return None;
            }
            return Some(p + n);
        }
        Some(end)
    }

    // ------------------------------------------------------------ xmltok_impl.c

    fn scan_comment(self, b: &[u8], mut p: usize, end: usize, next: &mut usize) -> i32 {
        let m = self.mbpc();
        if p != end {
            if !self.char_matches(b, p, b'-') {
                *next = p;
                return TOK_INVALID;
            }
            p += m;
            while p < end {
                match self.invalid_case(b, &mut p, end, next) {
                    Step::Return(t) => return t,
                    Step::Advanced => continue,
                    Step::Other => {}
                }
                if self.bt(b, p) == BT_MINUS {
                    p += m;
                    if p == end {
                        return TOK_PARTIAL;
                    }
                    if self.char_matches(b, p, b'-') {
                        p += m;
                        if p == end {
                            return TOK_PARTIAL;
                        }
                        if !self.char_matches(b, p, b'>') {
                            *next = p;
                            return TOK_INVALID;
                        }
                        *next = p + m;
                        return TOK_COMMENT;
                    }
                } else {
                    p += m;
                }
            }
        }
        TOK_PARTIAL
    }

    fn scan_decl(self, b: &[u8], mut p: usize, end: usize, next: &mut usize) -> i32 {
        let m = self.mbpc();
        if p == end {
            return TOK_PARTIAL;
        }
        match self.bt(b, p) {
            BT_MINUS => return self.scan_comment(b, p + m, end, next),
            BT_LSQB => {
                *next = p + m;
                return TOK_COND_SECT_OPEN;
            }
            BT_NMSTRT | BT_HEX => p += m,
            _ => {
                *next = p;
                return TOK_INVALID;
            }
        }
        while p < end {
            match self.bt(b, p) {
                BT_PERCNT => {
                    if p + m == end {
                        return TOK_PARTIAL;
                    }
                    *next = p;
                    if matches!(self.bt(b, p + m), BT_S | BT_CR | BT_LF | BT_PERCNT) {
                        return TOK_INVALID;
                    }
                    return TOK_DECL_OPEN;
                }
                BT_S | BT_CR | BT_LF => {
                    *next = p;
                    return TOK_DECL_OPEN;
                }
                BT_NMSTRT | BT_HEX => p += m,
                _ => {
                    *next = p;
                    return TOK_INVALID;
                }
            }
        }
        TOK_PARTIAL
    }

    /// `checkPiTarget`: (whether the target may be used, the token: PI or XML_DECL).
    fn check_pi_target(self, b: &[u8], mut p: usize, end: usize) -> (bool, i32) {
        let m = self.mbpc();
        let mut upper = false;
        if end - p != m * 3 {
            return (true, TOK_PI);
        }
        for (lo, up) in [(b'x', b'X'), (b'm', b'M'), (b'l', b'L')] {
            let c = self.byte_to_ascii(b, p);
            if c == up as i32 {
                upper = true;
            } else if c != lo as i32 {
                return (true, TOK_PI);
            }
            p += m;
        }
        if upper {
            return (false, TOK_PI);
        }
        (true, TOK_XML_DECL)
    }

    fn scan_pi(self, b: &[u8], mut p: usize, end: usize, next: &mut usize) -> i32 {
        let m = self.mbpc();
        let target = p;
        if p == end {
            return TOK_PARTIAL;
        }
        match self.nmstrt_case(b, &mut p, end, next) {
            Step::Return(t) => return t,
            Step::Advanced => {}
            Step::Other => {
                *next = p;
                return TOK_INVALID;
            }
        }
        while p < end {
            match self.name_case(b, &mut p, end, next) {
                Step::Return(t) => return t,
                Step::Advanced => continue,
                Step::Other => {}
            }
            match self.bt(b, p) {
                BT_S | BT_CR | BT_LF => {
                    let (ok, tok) = self.check_pi_target(b, target, p);
                    if !ok {
                        *next = p;
                        return TOK_INVALID;
                    }
                    p += m;
                    while p < end {
                        match self.invalid_case(b, &mut p, end, next) {
                            Step::Return(t) => return t,
                            Step::Advanced => continue,
                            Step::Other => {}
                        }
                        if self.bt(b, p) == BT_QUEST {
                            p += m;
                            if p == end {
                                return TOK_PARTIAL;
                            }
                            if self.char_matches(b, p, b'>') {
                                *next = p + m;
                                return tok;
                            }
                        } else {
                            p += m;
                        }
                    }
                    return TOK_PARTIAL;
                }
                BT_QUEST => {
                    let (ok, tok) = self.check_pi_target(b, target, p);
                    if !ok {
                        *next = p;
                        return TOK_INVALID;
                    }
                    p += m;
                    if p == end {
                        return TOK_PARTIAL;
                    }
                    if self.char_matches(b, p, b'>') {
                        *next = p + m;
                        return tok;
                    }
                    *next = p;
                    return TOK_INVALID;
                }
                _ => {
                    *next = p;
                    return TOK_INVALID;
                }
            }
        }
        TOK_PARTIAL
    }

    fn scan_cdata_section(self, b: &[u8], mut p: usize, end: usize, next: &mut usize) -> i32 {
        let m = self.mbpc();
        if end - p < 6 * m {
            return TOK_PARTIAL;
        }
        for c in *b"CDATA[" {
            if !self.char_matches(b, p, c) {
                *next = p;
                return TOK_INVALID;
            }
            p += m;
        }
        *next = p;
        TOK_CDATA_SECT_OPEN
    }

    /// `cdataSectionTok`.
    pub(super) fn cdata_section_tok(
        self,
        b: &[u8],
        mut p: usize,
        end: usize,
        next: &mut usize,
    ) -> i32 {
        let m = self.mbpc();
        if p == end {
            return TOK_NONE;
        }
        let Some(end) = self.whole_units(p, end) else {
            return TOK_PARTIAL;
        };
        match self.bt(b, p) {
            BT_RSQB => {
                p += m;
                if p == end {
                    return TOK_PARTIAL;
                }
                if self.char_matches(b, p, b']') {
                    p += m;
                    if p == end {
                        return TOK_PARTIAL;
                    }
                    if self.char_matches(b, p, b'>') {
                        *next = p + m;
                        return TOK_CDATA_SECT_CLOSE;
                    }
                    p -= m;
                }
            }
            BT_CR => {
                p += m;
                if p == end {
                    return TOK_PARTIAL;
                }
                if self.bt(b, p) == BT_LF {
                    p += m;
                }
                *next = p;
                return TOK_DATA_NEWLINE;
            }
            BT_LF => {
                *next = p + m;
                return TOK_DATA_NEWLINE;
            }
            _ => match self.invalid_case(b, &mut p, end, next) {
                Step::Return(t) => return t,
                Step::Advanced => {}
                Step::Other => p += m,
            },
        }
        while p < end {
            match self.bt(b, p) {
                t @ (BT_LEAD2 | BT_LEAD3 | BT_LEAD4) => {
                    let n = lead(t);
                    if end - p < n || self.is_invalid(b, p, n) {
                        *next = p;
                        return TOK_DATA_CHARS;
                    }
                    p += n;
                }
                BT_NONXML | BT_MALFORM | BT_TRAIL | BT_CR | BT_LF | BT_RSQB => {
                    *next = p;
                    return TOK_DATA_CHARS;
                }
                _ => p += m,
            }
        }
        *next = p;
        TOK_DATA_CHARS
    }

    fn scan_end_tag(self, b: &[u8], mut p: usize, end: usize, next: &mut usize) -> i32 {
        let m = self.mbpc();
        if p == end {
            return TOK_PARTIAL;
        }
        match self.nmstrt_case(b, &mut p, end, next) {
            Step::Return(t) => return t,
            Step::Advanced => {}
            Step::Other => {
                *next = p;
                return TOK_INVALID;
            }
        }
        while p < end {
            match self.name_case(b, &mut p, end, next) {
                Step::Return(t) => return t,
                Step::Advanced => continue,
                Step::Other => {}
            }
            match self.bt(b, p) {
                BT_S | BT_CR | BT_LF => {
                    p += m;
                    while p < end {
                        match self.bt(b, p) {
                            BT_S | BT_CR | BT_LF => {}
                            BT_GT => {
                                *next = p + m;
                                return TOK_END_TAG;
                            }
                            _ => {
                                *next = p;
                                return TOK_INVALID;
                            }
                        }
                        p += m;
                    }
                    return TOK_PARTIAL;
                }
                BT_GT => {
                    *next = p + m;
                    return TOK_END_TAG;
                }
                _ => {
                    *next = p;
                    return TOK_INVALID;
                }
            }
        }
        TOK_PARTIAL
    }

    fn scan_hex_char_ref(self, b: &[u8], mut p: usize, end: usize, next: &mut usize) -> i32 {
        let m = self.mbpc();
        if p != end {
            if !matches!(self.bt(b, p), BT_DIGIT | BT_HEX) {
                *next = p;
                return TOK_INVALID;
            }
            p += m;
            while p < end {
                match self.bt(b, p) {
                    BT_DIGIT | BT_HEX => {}
                    BT_SEMI => {
                        *next = p + m;
                        return TOK_CHAR_REF;
                    }
                    _ => {
                        *next = p;
                        return TOK_INVALID;
                    }
                }
                p += m;
            }
        }
        TOK_PARTIAL
    }

    fn scan_char_ref(self, b: &[u8], mut p: usize, end: usize, next: &mut usize) -> i32 {
        let m = self.mbpc();
        if p != end {
            if self.char_matches(b, p, b'x') {
                return self.scan_hex_char_ref(b, p + m, end, next);
            }
            if self.bt(b, p) != BT_DIGIT {
                *next = p;
                return TOK_INVALID;
            }
            p += m;
            while p < end {
                match self.bt(b, p) {
                    BT_DIGIT => {}
                    BT_SEMI => {
                        *next = p + m;
                        return TOK_CHAR_REF;
                    }
                    _ => {
                        *next = p;
                        return TOK_INVALID;
                    }
                }
                p += m;
            }
        }
        TOK_PARTIAL
    }

    fn scan_ref(self, b: &[u8], mut p: usize, end: usize, next: &mut usize) -> i32 {
        let m = self.mbpc();
        if p == end {
            return TOK_PARTIAL;
        }
        match self.nmstrt_case(b, &mut p, end, next) {
            Step::Return(t) => return t,
            Step::Advanced => {}
            Step::Other => {
                if self.bt(b, p) == BT_NUM {
                    return self.scan_char_ref(b, p + m, end, next);
                }
                *next = p;
                return TOK_INVALID;
            }
        }
        while p < end {
            match self.name_case(b, &mut p, end, next) {
                Step::Return(t) => return t,
                Step::Advanced => continue,
                Step::Other => {}
            }
            if self.bt(b, p) == BT_SEMI {
                *next = p + m;
                return TOK_ENTITY_REF;
            }
            *next = p;
            return TOK_INVALID;
        }
        TOK_PARTIAL
    }

    fn scan_atts(self, b: &[u8], mut p: usize, end: usize, next: &mut usize) -> i32 {
        let m = self.mbpc();
        while p < end {
            match self.name_case(b, &mut p, end, next) {
                Step::Return(t) => return t,
                Step::Advanced => continue,
                Step::Other => {}
            }
            let t = self.bt(b, p);
            if !matches!(t, BT_S | BT_CR | BT_LF | BT_EQUALS) {
                *next = p;
                return TOK_INVALID;
            }
            if t != BT_EQUALS {
                loop {
                    p += m;
                    if p == end {
                        return TOK_PARTIAL;
                    }
                    let t = self.bt(b, p);
                    if t == BT_EQUALS {
                        break;
                    }
                    if !matches!(t, BT_S | BT_LF | BT_CR) {
                        *next = p;
                        return TOK_INVALID;
                    }
                }
            }
            let open;
            loop {
                p += m;
                if p == end {
                    return TOK_PARTIAL;
                }
                let o = self.bt(b, p);
                if o == BT_QUOT || o == BT_APOS {
                    open = o;
                    break;
                }
                if !matches!(o, BT_S | BT_LF | BT_CR) {
                    *next = p;
                    return TOK_INVALID;
                }
            }
            p += m;
            loop {
                if p == end {
                    return TOK_PARTIAL;
                }
                let t = self.bt(b, p);
                if t == open {
                    break;
                }
                match self.invalid_case(b, &mut p, end, next) {
                    Step::Return(t) => return t,
                    Step::Advanced => continue,
                    Step::Other => {}
                }
                match t {
                    BT_AMP => {
                        let mut q = p;
                        let tok = self.scan_ref(b, p + m, end, &mut q);
                        if tok <= 0 {
                            if tok == TOK_INVALID {
                                *next = q;
                            }
                            return tok;
                        }
                        p = q;
                    }
                    BT_LT => {
                        *next = p;
                        return TOK_INVALID;
                    }
                    _ => p += m,
                }
            }
            p += m;
            if p == end {
                return TOK_PARTIAL;
            }
            match self.bt(b, p) {
                BT_S | BT_CR | BT_LF => {}
                BT_SOL => return self.sol(b, p, end, next, TOK_EMPTY_ELEMENT_WITH_ATTS),
                BT_GT => {
                    *next = p + m;
                    return TOK_START_TAG_WITH_ATTS;
                }
                _ => {
                    *next = p;
                    return TOK_INVALID;
                }
            }
            loop {
                p += m;
                if p == end {
                    return TOK_PARTIAL;
                }
                match self.nmstrt_case(b, &mut p, end, next) {
                    Step::Return(t) => return t,
                    Step::Advanced => break,
                    Step::Other => {}
                }
                match self.bt(b, p) {
                    BT_S | BT_CR | BT_LF => continue,
                    BT_GT => {
                        *next = p + m;
                        return TOK_START_TAG_WITH_ATTS;
                    }
                    BT_SOL => return self.sol(b, p, end, next, TOK_EMPTY_ELEMENT_WITH_ATTS),
                    _ => {
                        *next = p;
                        return TOK_INVALID;
                    }
                }
            }
        }
        TOK_PARTIAL
    }

    /// The `sol:` label: '/' then '>'.
    fn sol(self, b: &[u8], mut p: usize, end: usize, next: &mut usize, tok: i32) -> i32 {
        let m = self.mbpc();
        p += m;
        if p == end {
            return TOK_PARTIAL;
        }
        if !self.char_matches(b, p, b'>') {
            *next = p;
            return TOK_INVALID;
        }
        *next = p + m;
        tok
    }

    fn scan_lt(self, b: &[u8], mut p: usize, end: usize, next: &mut usize) -> i32 {
        let m = self.mbpc();
        if p == end {
            return TOK_PARTIAL;
        }
        match self.nmstrt_case(b, &mut p, end, next) {
            Step::Return(t) => return t,
            Step::Advanced => {}
            Step::Other => match self.bt(b, p) {
                BT_EXCL => {
                    p += m;
                    if p == end {
                        return TOK_PARTIAL;
                    }
                    match self.bt(b, p) {
                        BT_MINUS => return self.scan_comment(b, p + m, end, next),
                        BT_LSQB => return self.scan_cdata_section(b, p + m, end, next),
                        _ => {}
                    }
                    *next = p;
                    return TOK_INVALID;
                }
                BT_QUEST => return self.scan_pi(b, p + m, end, next),
                BT_SOL => return self.scan_end_tag(b, p + m, end, next),
                _ => {
                    *next = p;
                    return TOK_INVALID;
                }
            },
        }
        while p < end {
            match self.name_case(b, &mut p, end, next) {
                Step::Return(t) => return t,
                Step::Advanced => continue,
                Step::Other => {}
            }
            match self.bt(b, p) {
                BT_S | BT_CR | BT_LF => {
                    p += m;
                    while p < end {
                        match self.nmstrt_case(b, &mut p, end, next) {
                            Step::Return(t) => return t,
                            Step::Advanced => return self.scan_atts(b, p, end, next),
                            Step::Other => {}
                        }
                        match self.bt(b, p) {
                            BT_GT => {
                                *next = p + m;
                                return TOK_START_TAG_NO_ATTS;
                            }
                            BT_SOL => return self.sol(b, p, end, next, TOK_EMPTY_ELEMENT_NO_ATTS),
                            BT_S | BT_CR | BT_LF => p += m,
                            _ => {
                                *next = p;
                                return TOK_INVALID;
                            }
                        }
                    }
                    return TOK_PARTIAL;
                }
                BT_GT => {
                    *next = p + m;
                    return TOK_START_TAG_NO_ATTS;
                }
                BT_SOL => return self.sol(b, p, end, next, TOK_EMPTY_ELEMENT_NO_ATTS),
                _ => {
                    *next = p;
                    return TOK_INVALID;
                }
            }
        }
        TOK_PARTIAL
    }

    /// `contentTok`.
    pub(super) fn content_tok(self, b: &[u8], mut p: usize, end: usize, next: &mut usize) -> i32 {
        let m = self.mbpc();
        if p == end {
            return TOK_NONE;
        }
        let Some(end) = self.whole_units(p, end) else {
            return TOK_PARTIAL;
        };
        match self.bt(b, p) {
            BT_LT => return self.scan_lt(b, p + m, end, next),
            BT_AMP => return self.scan_ref(b, p + m, end, next),
            BT_CR => {
                p += m;
                if p == end {
                    return TOK_TRAILING_CR;
                }
                if self.bt(b, p) == BT_LF {
                    p += m;
                }
                *next = p;
                return TOK_DATA_NEWLINE;
            }
            BT_LF => {
                *next = p + m;
                return TOK_DATA_NEWLINE;
            }
            BT_RSQB => {
                p += m;
                if p == end {
                    return TOK_TRAILING_RSQB;
                }
                if self.char_matches(b, p, b']') {
                    p += m;
                    if p == end {
                        return TOK_TRAILING_RSQB;
                    }
                    if self.char_matches(b, p, b'>') {
                        *next = p;
                        return TOK_INVALID;
                    }
                    p -= m;
                }
            }
            _ => match self.invalid_case(b, &mut p, end, next) {
                Step::Return(t) => return t,
                Step::Advanced => {}
                Step::Other => p += m,
            },
        }
        while p < end {
            match self.bt(b, p) {
                t @ (BT_LEAD2 | BT_LEAD3 | BT_LEAD4) => {
                    let n = lead(t);
                    if end - p < n || self.is_invalid(b, p, n) {
                        *next = p;
                        return TOK_DATA_CHARS;
                    }
                    p += n;
                }
                BT_RSQB => {
                    if p + m != end {
                        if !self.char_matches(b, p + m, b']') {
                            p += m;
                            continue;
                        }
                        if p + 2 * m != end {
                            if !self.char_matches(b, p + 2 * m, b'>') {
                                p += m;
                                continue;
                            }
                            *next = p + 2 * m;
                            return TOK_INVALID;
                        }
                    }
                    *next = p;
                    return TOK_DATA_CHARS;
                }
                BT_AMP | BT_LT | BT_NONXML | BT_MALFORM | BT_TRAIL | BT_CR | BT_LF => {
                    *next = p;
                    return TOK_DATA_CHARS;
                }
                _ => p += m,
            }
        }
        *next = p;
        TOK_DATA_CHARS
    }

    fn scan_percent(self, b: &[u8], mut p: usize, end: usize, next: &mut usize) -> i32 {
        let m = self.mbpc();
        if p == end {
            return -TOK_PERCENT;
        }
        match self.nmstrt_case(b, &mut p, end, next) {
            Step::Return(t) => return t,
            Step::Advanced => {}
            Step::Other => {
                *next = p;
                if matches!(self.bt(b, p), BT_S | BT_LF | BT_CR | BT_PERCNT) {
                    return TOK_PERCENT;
                }
                return TOK_INVALID;
            }
        }
        while p < end {
            match self.name_case(b, &mut p, end, next) {
                Step::Return(t) => return t,
                Step::Advanced => continue,
                Step::Other => {}
            }
            if self.bt(b, p) == BT_SEMI {
                *next = p + m;
                return TOK_PARAM_ENTITY_REF;
            }
            *next = p;
            return TOK_INVALID;
        }
        TOK_PARTIAL
    }

    fn scan_pound_name(self, b: &[u8], mut p: usize, end: usize, next: &mut usize) -> i32 {
        if p == end {
            return TOK_PARTIAL;
        }
        match self.nmstrt_case(b, &mut p, end, next) {
            Step::Return(t) => return t,
            Step::Advanced => {}
            Step::Other => {
                *next = p;
                return TOK_INVALID;
            }
        }
        while p < end {
            match self.name_case(b, &mut p, end, next) {
                Step::Return(t) => return t,
                Step::Advanced => continue,
                Step::Other => {}
            }
            *next = p;
            if matches!(
                self.bt(b, p),
                BT_CR | BT_LF | BT_S | BT_RPAR | BT_GT | BT_PERCNT | BT_VERBAR
            ) {
                return TOK_POUND_NAME;
            }
            return TOK_INVALID;
        }
        -TOK_POUND_NAME
    }

    fn scan_lit(self, open: u8, b: &[u8], mut p: usize, end: usize, next: &mut usize) -> i32 {
        let m = self.mbpc();
        while p < end {
            match self.invalid_case(b, &mut p, end, next) {
                Step::Return(t) => return t,
                Step::Advanced => continue,
                Step::Other => {}
            }
            let t = self.bt(b, p);
            p += m;
            if t == BT_QUOT || t == BT_APOS {
                if t != open {
                    continue;
                }
                if p == end {
                    return -TOK_LITERAL;
                }
                *next = p;
                return match self.bt(b, p) {
                    BT_S | BT_CR | BT_LF | BT_GT | BT_PERCNT | BT_LSQB => TOK_LITERAL,
                    _ => TOK_INVALID,
                };
            }
        }
        TOK_PARTIAL
    }

    /// `prologTok`.
    pub(super) fn prolog_tok(self, b: &[u8], mut p: usize, end: usize, next: &mut usize) -> i32 {
        let m = self.mbpc();
        if p == end {
            return TOK_NONE;
        }
        let Some(end) = self.whole_units(p, end) else {
            return TOK_PARTIAL;
        };
        let mut tok;
        match self.bt(b, p) {
            BT_QUOT => return self.scan_lit(BT_QUOT, b, p + m, end, next),
            BT_APOS => return self.scan_lit(BT_APOS, b, p + m, end, next),
            BT_LT => {
                p += m;
                if p == end {
                    return TOK_PARTIAL;
                }
                match self.bt(b, p) {
                    BT_EXCL => return self.scan_decl(b, p + m, end, next),
                    BT_QUEST => return self.scan_pi(b, p + m, end, next),
                    BT_NMSTRT | BT_HEX | BT_NONASCII | BT_LEAD2 | BT_LEAD3 | BT_LEAD4 => {
                        *next = p - m;
                        return TOK_INSTANCE_START;
                    }
                    _ => {}
                }
                *next = p;
                return TOK_INVALID;
            }
            t @ (BT_CR | BT_S | BT_LF) => {
                if t == BT_CR && p + m == end {
                    *next = end;
                    return -TOK_PROLOG_S;
                }
                loop {
                    p += m;
                    if p == end {
                        break;
                    }
                    match self.bt(b, p) {
                        BT_S | BT_LF => {}
                        BT_CR if p + m != end => {}
                        _ => {
                            *next = p;
                            return TOK_PROLOG_S;
                        }
                    }
                }
                *next = p;
                return TOK_PROLOG_S;
            }
            BT_PERCNT => return self.scan_percent(b, p + m, end, next),
            BT_COMMA => {
                *next = p + m;
                return TOK_COMMA;
            }
            BT_LSQB => {
                *next = p + m;
                return TOK_OPEN_BRACKET;
            }
            BT_RSQB => {
                p += m;
                if p == end {
                    return -TOK_CLOSE_BRACKET;
                }
                if self.char_matches(b, p, b']') {
                    if p + m == end {
                        return TOK_PARTIAL;
                    }
                    if self.char_matches(b, p + m, b'>') {
                        *next = p + 2 * m;
                        return TOK_COND_SECT_CLOSE;
                    }
                }
                *next = p;
                return TOK_CLOSE_BRACKET;
            }
            BT_LPAR => {
                *next = p + m;
                return TOK_OPEN_PAREN;
            }
            BT_RPAR => {
                p += m;
                if p == end {
                    return -TOK_CLOSE_PAREN;
                }
                let r = match self.bt(b, p) {
                    BT_AST => TOK_CLOSE_PAREN_ASTERISK,
                    BT_QUEST => TOK_CLOSE_PAREN_QUESTION,
                    BT_PLUS => TOK_CLOSE_PAREN_PLUS,
                    BT_CR | BT_LF | BT_S | BT_GT | BT_COMMA | BT_VERBAR | BT_RPAR => {
                        *next = p;
                        return TOK_CLOSE_PAREN;
                    }
                    _ => {
                        *next = p;
                        return TOK_INVALID;
                    }
                };
                *next = p + m;
                return r;
            }
            BT_VERBAR => {
                *next = p + m;
                return TOK_OR;
            }
            BT_GT => {
                *next = p + m;
                return TOK_DECL_CLOSE;
            }
            BT_NUM => return self.scan_pound_name(b, p + m, end, next),
            t @ (BT_LEAD2 | BT_LEAD3 | BT_LEAD4) => {
                let n = lead(t);
                if end - p < n {
                    return TOK_PARTIAL_CHAR;
                }
                if self.is_nmstrt(b, p, n) {
                    p += n;
                    tok = TOK_NAME;
                } else if self.is_name(b, p, n) {
                    p += n;
                    tok = TOK_NMTOKEN;
                } else {
                    *next = p;
                    return TOK_INVALID;
                }
            }
            BT_NMSTRT | BT_HEX => {
                tok = TOK_NAME;
                p += m;
            }
            BT_DIGIT | BT_NAME | BT_MINUS => {
                tok = TOK_NMTOKEN;
                p += m;
            }
            BT_NONASCII => {
                if self.is_nmstrt_min(b, p) {
                    p += m;
                    tok = TOK_NAME;
                } else if self.is_name_min(b, p) {
                    p += m;
                    tok = TOK_NMTOKEN;
                } else {
                    *next = p;
                    return TOK_INVALID;
                }
            }
            _ => {
                *next = p;
                return TOK_INVALID;
            }
        }
        while p < end {
            match self.name_case(b, &mut p, end, next) {
                Step::Return(t) => return t,
                Step::Advanced => continue,
                Step::Other => {}
            }
            match self.bt(b, p) {
                BT_GT | BT_RPAR | BT_COMMA | BT_VERBAR | BT_LSQB | BT_PERCNT | BT_S | BT_CR
                | BT_LF => {
                    *next = p;
                    return tok;
                }
                t @ (BT_PLUS | BT_AST | BT_QUEST) => {
                    *next = p;
                    if tok == TOK_NMTOKEN {
                        return TOK_INVALID;
                    }
                    *next = p + m;
                    tok = match t {
                        BT_PLUS => TOK_NAME_PLUS,
                        BT_AST => TOK_NAME_ASTERISK,
                        _ => TOK_NAME_QUESTION,
                    };
                    return tok;
                }
                _ => {
                    *next = p;
                    return TOK_INVALID;
                }
            }
        }
        -tok
    }

    /// `attributeValueTok`.
    pub(super) fn attribute_value_tok(
        self,
        b: &[u8],
        mut p: usize,
        end: usize,
        next: &mut usize,
    ) -> i32 {
        let m = self.mbpc();
        if p == end {
            return TOK_NONE;
        }
        let start = p;
        while p < end {
            match self.bt(b, p) {
                t @ (BT_LEAD2 | BT_LEAD3 | BT_LEAD4) => p += lead(t),
                BT_AMP => {
                    if p == start {
                        return self.scan_ref(b, p + m, end, next);
                    }
                    *next = p;
                    return TOK_DATA_CHARS;
                }
                BT_LT => {
                    *next = p;
                    return TOK_INVALID;
                }
                BT_LF => {
                    if p == start {
                        *next = p + m;
                        return TOK_DATA_NEWLINE;
                    }
                    *next = p;
                    return TOK_DATA_CHARS;
                }
                BT_CR => {
                    if p == start {
                        p += m;
                        if p == end {
                            return TOK_TRAILING_CR;
                        }
                        if self.bt(b, p) == BT_LF {
                            p += m;
                        }
                        *next = p;
                        return TOK_DATA_NEWLINE;
                    }
                    *next = p;
                    return TOK_DATA_CHARS;
                }
                BT_S => {
                    if p == start {
                        *next = p + m;
                        return TOK_ATTRIBUTE_VALUE_S;
                    }
                    *next = p;
                    return TOK_DATA_CHARS;
                }
                _ => p += m,
            }
        }
        *next = p;
        TOK_DATA_CHARS
    }

    /// `entityValueTok`.
    pub(super) fn entity_value_tok(
        self,
        b: &[u8],
        mut p: usize,
        end: usize,
        next: &mut usize,
    ) -> i32 {
        let m = self.mbpc();
        if p == end {
            return TOK_NONE;
        }
        let start = p;
        while p < end {
            match self.bt(b, p) {
                t @ (BT_LEAD2 | BT_LEAD3 | BT_LEAD4) => p += lead(t),
                BT_AMP => {
                    if p == start {
                        return self.scan_ref(b, p + m, end, next);
                    }
                    *next = p;
                    return TOK_DATA_CHARS;
                }
                BT_PERCNT => {
                    if p == start {
                        let tok = self.scan_percent(b, p + m, end, next);
                        return if tok == TOK_PERCENT { TOK_INVALID } else { tok };
                    }
                    *next = p;
                    return TOK_DATA_CHARS;
                }
                BT_LF => {
                    if p == start {
                        *next = p + m;
                        return TOK_DATA_NEWLINE;
                    }
                    *next = p;
                    return TOK_DATA_CHARS;
                }
                BT_CR => {
                    if p == start {
                        p += m;
                        if p == end {
                            return TOK_TRAILING_CR;
                        }
                        if self.bt(b, p) == BT_LF {
                            p += m;
                        }
                        *next = p;
                        return TOK_DATA_NEWLINE;
                    }
                    *next = p;
                    return TOK_DATA_CHARS;
                }
                _ => p += m,
            }
        }
        *next = p;
        TOK_DATA_CHARS
    }

    /// `isPublicId` of a literal (its quotes included): `Err(the bad character's position)`.
    pub(super) fn is_public_id(self, b: &[u8], p: usize, end: usize) -> Result<(), usize> {
        let m = self.mbpc();
        let mut p = p + m;
        let end = end - m;
        while p < end {
            match self.bt(b, p) {
                BT_DIGIT | BT_HEX | BT_MINUS | BT_APOS | BT_LPAR | BT_RPAR | BT_PLUS | BT_COMMA
                | BT_SOL | BT_EQUALS | BT_QUEST | BT_CR | BT_LF | BT_SEMI | BT_EXCL | BT_AST
                | BT_PERCNT | BT_NUM => {}
                BT_S => {
                    if self.char_matches(b, p, b'\t') {
                        return Err(p);
                    }
                }
                t => {
                    let c = self.byte_to_ascii(b, p);
                    let plain = matches!(t, BT_NAME | BT_NMSTRT) && (c & !0x7f) == 0;
                    if !plain && c != 0x24 && c != 0x40 {
                        return Err(p);
                    }
                }
            }
            p += m;
        }
        Ok(())
    }

    /// `getAtts` of a well-formed start tag at `p` (its '<'): each attribute's name start,
    /// value start and end, and whether its value needs no normalization.
    pub(super) fn get_atts(self, b: &[u8], mut p: usize) -> Vec<Att> {
        #[derive(PartialEq)]
        enum St {
            Other,
            InName,
            InValue,
        }
        let m = self.mbpc();
        let mut state = St::InName;
        let mut atts: Vec<Att> = Vec::new();
        let mut cur = Att::default();
        let mut open = 0u8;
        p += m;
        loop {
            let t = self.bt(b, p);
            match t {
                BT_LEAD2 | BT_LEAD3 | BT_LEAD4 | BT_NONASCII | BT_NMSTRT | BT_HEX => {
                    if state == St::Other {
                        cur = Att {
                            name: p,
                            normalized: true,
                            ..Att::default()
                        };
                        state = St::InName;
                    }
                    if matches!(t, BT_LEAD2 | BT_LEAD3 | BT_LEAD4) {
                        p += lead(t) - m;
                    }
                }
                BT_QUOT | BT_APOS => {
                    if state != St::InValue {
                        cur.value = p + m;
                        state = St::InValue;
                        open = t;
                    } else if open == t {
                        state = St::Other;
                        cur.value_end = p;
                        atts.push(cur.clone());
                    }
                }
                BT_AMP => cur.normalized = false,
                BT_S => {
                    if state == St::InName {
                        state = St::Other;
                    } else if state == St::InValue
                        && cur.normalized
                        && (p == cur.value
                            || self.byte_to_ascii(b, p) != 0x20
                            || self.byte_to_ascii(b, p + m) == 0x20
                            || self.bt(b, p + m) == open)
                    {
                        cur.normalized = false;
                    }
                }
                BT_CR | BT_LF => {
                    if state == St::InName {
                        state = St::Other;
                    } else if state == St::InValue {
                        cur.normalized = false;
                    }
                }
                BT_GT | BT_SOL if state != St::InValue => return atts,
                _ => {}
            }
            p += m;
        }
    }

    /// `charRefNumber` of a character reference at `p` (its '&'): the code point, or -1.
    pub(super) fn char_ref_number(self, b: &[u8], p: usize) -> i32 {
        let m = self.mbpc();
        let mut p = p + 2 * m;
        let mut result: i32 = 0;
        if self.char_matches(b, p, b'x') {
            p += m;
            while !self.char_matches(b, p, b';') {
                let c = self.byte_to_ascii(b, p);
                if (0x30..=0x39).contains(&c) {
                    result = (result << 4) | (c - 0x30);
                } else if (0x41..=0x46).contains(&c) {
                    result = (result << 4) + 10 + (c - 0x41);
                } else if (0x61..=0x66).contains(&c) {
                    result = (result << 4) + 10 + (c - 0x61);
                }
                if result >= 0x110000 {
                    return -1;
                }
                p += m;
            }
        } else {
            while !self.char_matches(b, p, b';') {
                let c = self.byte_to_ascii(b, p);
                result = result * 10 + (c - 0x30);
                if result >= 0x110000 {
                    return -1;
                }
                p += m;
            }
        }
        check_char_ref_number(result)
    }

    /// `predefinedEntityName` of the name `b[p..end]`: the character, or 0.
    pub(super) fn predefined_entity_name(self, b: &[u8], p: usize, end: usize) -> u8 {
        let m = self.mbpc();
        let is = |k: usize, c: u8| self.char_matches(b, p + k * m, c);
        match (end - p) / m {
            2 if is(1, b't') => match self.byte_to_ascii(b, p) {
                0x6C => b'<',
                0x67 => b'>',
                _ => 0,
            },
            3 if is(0, b'a') && is(1, b'm') && is(2, b'p') => b'&',
            4 => match self.byte_to_ascii(b, p) {
                0x71 if is(1, b'u') && is(2, b'o') && is(3, b't') => b'"',
                0x61 if is(1, b'p') && is(2, b'o') && is(3, b's') => b'\'',
                _ => 0,
            },
            _ => 0,
        }
    }

    /// `nameMatchesAscii`.
    pub(super) fn name_matches_ascii(self, b: &[u8], mut p: usize, end: usize, kw: &[u8]) -> bool {
        for &c in kw {
            if p >= end || !self.char_matches(b, p, c) {
                return false;
            }
            p += self.mbpc();
        }
        p == end
    }

    /// `nameLength`.
    pub(super) fn name_length(self, b: &[u8], mut p: usize) -> usize {
        let start = p;
        loop {
            match self.bt(b, p) {
                t @ (BT_LEAD2 | BT_LEAD3 | BT_LEAD4) => p += lead(t),
                BT_NONASCII | BT_NMSTRT | BT_HEX | BT_DIGIT | BT_NAME | BT_MINUS => {
                    p += self.mbpc()
                }
                _ => return p - start,
            }
        }
    }

    /// `skipS`.
    pub(super) fn skip_s(self, b: &[u8], mut p: usize) -> usize {
        while matches!(self.bt(b, p), BT_LF | BT_CR | BT_S) {
            p += self.mbpc();
        }
        p
    }

    /// `updatePosition`: the lines (and columns) of `b[p..end]` added to `pos`; `false` when a
    /// character steps over `end` (expat's loop then runs past the document: larmorx stops).
    pub(super) fn update_position(
        self,
        b: &[u8],
        mut p: usize,
        end: usize,
        pos: &mut (u64, u64),
    ) -> bool {
        let m = self.mbpc();
        while p < end && p < b.len() {
            match self.bt(b, p) {
                t @ (BT_LEAD2 | BT_LEAD3 | BT_LEAD4) => p += lead(t),
                BT_LF => {
                    pos.1 = u64::MAX;
                    pos.0 += 1;
                    p += m;
                }
                BT_CR => {
                    pos.0 += 1;
                    p += m;
                    if p != end && self.bt(b, p) == BT_LF {
                        p += m;
                    }
                    pos.1 = u64::MAX;
                }
                _ => p += m,
            }
            pos.1 = pos.1.wrapping_add(1);
        }
        p == end
    }

    /// `XmlUtf8Convert` of `b[*from..from_lim]`, appending at most `room` bytes to `to`
    /// (`usize::MAX`: no limit); `*from` ends where the conversion stopped.
    pub(super) fn to_utf8(
        self,
        b: &[u8],
        from: &mut usize,
        from_lim: usize,
        to: &mut Vec<u8>,
        room: usize,
    ) {
        match self {
            Enc::Utf8 | Enc::Internal => {
                // utf8_toUtf8's "avoid copying partial characters": in expat 2.0.1 it keeps the
                // lead byte of a character whose trail bytes do not fit
                let mut lim = from_lim;
                if from_lim - *from > room {
                    lim = *from + room;
                    while lim > *from {
                        if b[lim - 1] & 0xc0 != 0x80 {
                            break;
                        }
                        lim -= 1;
                    }
                }
                to.extend_from_slice(&b[*from..lim]);
                *from = lim;
            }
            Enc::Ascii => {
                let n = (from_lim - *from).min(room);
                to.extend_from_slice(&b[*from..*from + n]);
                *from += n;
            }
            Enc::Latin1 => {
                let mut left = room;
                while *from != from_lim {
                    let c = b[*from];
                    if c & 0x80 != 0 {
                        if left < 2 {
                            break;
                        }
                        to.push((c >> 6) | 0xc0);
                        to.push((c & 0x3f) | 0x80);
                        left -= 2;
                    } else {
                        if left == 0 {
                            break;
                        }
                        to.push(c);
                        left -= 1;
                    }
                    *from += 1;
                }
            }
            Enc::Little2 | Enc::Big2 => {
                // (a unit cut by the end of the buffer reads as 0: expat reads past it)
                let at = |q: usize| b.get(q).copied().unwrap_or(0);
                let lohi = |q: usize| -> (u8, u8) {
                    if self == Enc::Little2 {
                        (at(q), at(q + 1))
                    } else {
                        (at(q + 1), at(q))
                    }
                };
                let mut left = room;
                let mut f = *from;
                // an odd byte at the end of a range is never converted (expat's loop steps
                // over the end there and reads past the document; larmorx stops)
                let whole = from_lim - ((from_lim - f) & 1);
                while f < whole {
                    let (lo, hi) = lohi(f);
                    match hi {
                        0 if lo < 0x80 => {
                            if left == 0 {
                                *from = f;
                                return;
                            }
                            to.push(lo);
                            left -= 1;
                        }
                        0..=7 => {
                            if left < 2 {
                                *from = f;
                                return;
                            }
                            to.push((lo >> 6) | (hi << 2) | 0xc0);
                            to.push((lo & 0x3f) | 0x80);
                            left -= 2;
                        }
                        0xD8..=0xDB => {
                            if left < 4 {
                                *from = f;
                                return;
                            }
                            let plane =
                                ((((hi & 0x3) as u32) << 2) | (((lo >> 6) & 0x3) as u32)) + 1;
                            to.push(((plane >> 2) as u8) | 0xf0);
                            to.push(((lo >> 2) & 0xF) | (((plane & 0x3) as u8) << 4) | 0x80);
                            f += 2;
                            let (lo2, hi2) = lohi(f);
                            to.push(((lo & 0x3) << 4) | ((hi2 & 0x3) << 2) | (lo2 >> 6) | 0x80);
                            to.push((lo2 & 0x3f) | 0x80);
                            left -= 4;
                        }
                        _ => {
                            if left < 3 {
                                *from = f;
                                return;
                            }
                            to.push((hi >> 4) | 0xe0);
                            to.push(((hi & 0xf) << 2) | (lo >> 6) | 0x80);
                            to.push((lo & 0x3f) | 0x80);
                            left -= 3;
                        }
                    }
                    f += 2;
                }
                *from = if f >= whole { from_lim } else { f };
            }
        }
    }

    /// `b[from..end]` converted to UTF-8 in full (`poolStoreString` without its terminator).
    pub(super) fn utf8(self, b: &[u8], from: usize, end: usize) -> Vec<u8> {
        let mut v = Vec::with_capacity(end - from);
        let mut f = from;
        self.to_utf8(b, &mut f, end, &mut v, usize::MAX);
        v
    }

    /// `toAscii`: the character at `p` converted into one byte (as a C `char`), or -1.
    pub(super) fn to_ascii(self, b: &[u8], p: usize, end: usize) -> i32 {
        let mut v = Vec::new();
        let mut f = p;
        self.to_utf8(b, &mut f, end, &mut v, 1);
        v.first().map_or(-1, |&c| c as i8 as i32)
    }
}

/// An attribute of `getAtts`.
#[derive(Clone, Debug, Default)]
pub(super) struct Att {
    pub name: usize,
    pub value: usize,
    pub value_end: usize,
    pub normalized: bool,
}

/// `checkCharRefNumber`.
pub(super) fn check_char_ref_number(result: i32) -> i32 {
    match result >> 8 {
        0xD8..=0xDF => -1,
        0 if T_LATIN1[result as usize] == BT_NONXML => -1,
        0xFF if result == 0xFFFE || result == 0xFFFF => -1,
        _ => result,
    }
}

/// `XmlUtf8Encode`: the bytes added to `out`.
pub(super) fn utf8_encode(c: i32, out: &mut Vec<u8>) -> usize {
    if c < 0 {
        return 0;
    }
    let c = c as u32;
    if c < 0x80 {
        out.push(c as u8);
        1
    } else if c < 0x800 {
        out.push(((c >> 6) | 0xc0) as u8);
        out.push(((c & 0x3f) | 0x80) as u8);
        2
    } else if c < 0x10000 {
        out.push(((c >> 12) | 0xe0) as u8);
        out.push((((c >> 6) & 0x3f) | 0x80) as u8);
        out.push(((c & 0x3f) | 0x80) as u8);
        3
    } else if c < 0x110000 {
        out.push(((c >> 18) | 0xf0) as u8);
        out.push((((c >> 12) & 0x3f) | 0x80) as u8);
        out.push((((c >> 6) & 0x3f) | 0x80) as u8);
        out.push(((c & 0x3f) | 0x80) as u8);
        4
    } else {
        0
    }
}

/// `getEncodingIndex` of an encoding name (`streqci`): ISO-8859-1, US-ASCII, UTF-8, UTF-16,
/// UTF-16BE, UTF-16LE as 0 to 5.
pub(super) fn encoding_index(name: &[u8]) -> Option<usize> {
    const NAMES: [&[u8]; 6] = [
        b"ISO-8859-1",
        b"US-ASCII",
        b"UTF-8",
        b"UTF-16",
        b"UTF-16BE",
        b"UTF-16LE",
    ];
    NAMES.iter().position(|n| n.eq_ignore_ascii_case(name))
}

/// The encodings of `encoding_index` (UTF-16 alone: big-endian).
pub(super) const ENCODINGS: [Enc; 6] = [
    Enc::Latin1,
    Enc::Ascii,
    Enc::Utf8,
    Enc::Big2,
    Enc::Big2,
    Enc::Little2,
];
