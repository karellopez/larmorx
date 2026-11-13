// SPDX-License-Identifier: Apache-2.0
//! Generated XML documents (every construct expat reports to gifticlib's handlers, in the
//! encodings expat reads, then mutated and cut) and the pieces they are fed in.

use larmorx_core::x87::cases::SplitMix64;

pub struct Gen {
    pub rng: SplitMix64,
    /// UTF-8 documents without an internal DTD subset only (`LXPROBE_XML_SIMPLE`).
    pub simple: bool,
}

const NAMES: &[&str] = &[
    "a",
    "b",
    "GIFTI",
    "MetaData",
    "MD",
    "Name",
    "Value",
    "x:y",
    "_u",
    "n.1",
    "Data",
    "e-f",
    "LabelTable",
    "\u{e9}t\u{e9}",
    "\u{4e2d}",
    "A\u{b7}B",
    "\u{2c00}",
    "\u{30fc}x",
    "\u{10000}",
    "\u{f900}a",
    "x\u{300}",
];
const TEXT: &[&str] = &[
    "hello",
    " ",
    "\n",
    "\r\n",
    "\r",
    "\t",
    "1 2 3",
    "&amp;",
    "&lt;",
    "&gt;",
    "&quot;",
    "&apos;",
    "&#65;",
    "&#x42;",
    "&#0;",
    "&#xD800;",
    "&#x10FFFF;",
    "&#x110000;",
    "&#;",
    "&#x;",
    "&e1;",
    "&undef;",
    "&pe;",
    "]",
    "]]",
    "]]>",
    "\u{e9}",
    "\u{20ac}",
    "\u{1f600}",
    "\u{fffe}",
    "\u{ffff}",
    "\x01",
    "\x7f",
    "a&b",
    "&",
    "<",
    ">",
    "'",
    "\"",
    "%",
    "&amp",
    "&#65",
    "  \n  ",
    "\r\r\n\n",
    "&e2;",
    "&ext;",
    "&notation;",
];

impl Gen {
    pub fn new(seed: u64) -> Gen {
        Gen {
            rng: SplitMix64::new(seed),
            simple: std::env::var("LXPROBE_XML_SIMPLE").is_ok(),
        }
    }
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.rng.next_u64() % n }
    }
    pub fn chance(&mut self, k: u64) -> bool {
        self.below(k) == 0
    }
    pub fn pick<'a>(&mut self, xs: &[&'a str]) -> &'a str {
        xs[self.below(xs.len() as u64) as usize]
    }

    fn ws(&mut self) -> &'static str {
        ["", " ", "\n", "  ", "\t", "\r\n", "\r"][self.below(7) as usize]
    }

    fn name(&mut self) -> String {
        if self.chance(25) {
            // an odd name: a digit or a punctuation first, a non-name character inside
            return self
                .pick(&[
                    "1a",
                    "-a",
                    ".a",
                    "a b",
                    "a\u{d7}",
                    "a/",
                    "\u{37e}",
                    "a\u{2028}",
                ])
                .to_string();
        }
        self.pick(NAMES).to_string()
    }

    fn quoted(&mut self, s: &str) -> String {
        if self.chance(2) {
            format!("\"{}\"", s.replace('"', "&quot;"))
        } else {
            format!("'{}'", s.replace('\'', "&apos;"))
        }
    }

    fn qpick(&mut self, xs: &[&str]) -> String {
        let v = self.pick(xs).to_string();
        self.quoted(&v)
    }

    fn attr_value(&mut self) -> String {
        let mut s = String::new();
        for _ in 0..self.below(4) {
            s += self.pick(TEXT);
        }
        s
    }

    fn element(&mut self, depth: u32, out: &mut String) {
        let name = self.name();
        out.push('<');
        out.push_str(&name);
        let natt = self.below(4);
        let mut used: Vec<String> = Vec::new();
        for _ in 0..natt {
            let mut k = self.name();
            if self.chance(10) && !used.is_empty() {
                k = used[0].clone(); // a duplicate
            }
            used.push(k.clone());
            let v = self.attr_value();
            out.push_str(&format!(
                " {k}{}={}{}",
                self.ws(),
                self.ws(),
                self.quoted(&v)
            ));
        }
        out.push_str(self.ws());
        if self.chance(5) {
            out.push_str("/>");
            return;
        }
        out.push('>');
        let n = if depth > 3 { 0 } else { self.below(5) };
        for _ in 0..n {
            match self.below(10) {
                0..=2 => self.element(depth + 1, out),
                3..=5 => {
                    for _ in 0..1 + self.below(4) {
                        out.push_str(self.pick(TEXT));
                    }
                }
                6 => out.push_str(&format!(
                    "<![CDATA[{}{}]]>",
                    self.pick(TEXT),
                    self.pick(&["", "]", "]]", "x]]y", "<a>", "\r\n"])
                )),
                7 => out.push_str(&format!(
                    "<!--{}{}-->",
                    self.pick(TEXT),
                    self.pick(&["", "-", " -x", "\r\n"])
                )),
                8 => out.push_str(&format!(
                    "<?{}{}{}?>",
                    self.pick(&["pi", "xml", "XmL", "x-y", "p"]),
                    self.pick(&["", " ", "\n"]),
                    self.pick(TEXT)
                )),
                _ => out.push_str(&"0123456789 ".repeat(1 + self.below(300) as usize)),
            }
        }
        if self.chance(30) {
            out.push_str(&format!("</{}>", self.name())); // possibly mismatched
        } else {
            out.push_str(&format!("</{name}{}>", self.ws()));
        }
    }

    fn content_model(&mut self) -> String {
        match self.below(8) {
            0 => "EMPTY".into(),
            1 => "ANY".into(),
            2 => "(#PCDATA)".into(),
            3 => format!("(#PCDATA|{}|{})*", self.name(), self.name()),
            4 => format!("({}{})", self.name(), self.pick(&["", "?", "*", "+"])),
            5 => format!(
                "({},({}|{})+,{}?)",
                self.name(),
                self.name(),
                self.name(),
                self.name()
            ),
            6 => format!(
                "({}|{}){}",
                self.name(),
                self.name(),
                self.pick(&["", "?", "*", "+"])
            ),
            _ => self
                .pick(&["(", "(a,|b)", "(#PCDATA)+", "(a b)", "EMPTY x", "(a,b|c)"])
                .into(),
        }
    }

    fn internal_subset(&mut self) -> String {
        let mut s = String::new();
        for _ in 0..self.below(6) {
            s.push_str(self.ws());
            let item = match self.below(11) {
                0 | 1 => format!("<!ELEMENT {} {}>", self.name(), self.content_model()),
                2 | 3 => {
                    let def = match self.below(5) {
                        0 => "#REQUIRED".to_string(),
                        1 => "#IMPLIED".to_string(),
                        2 => {
                            let v = self.attr_value_plain();
                            format!("#FIXED {}", self.quoted(&v))
                        }
                        _ => {
                            let v = self.attr_value_plain();
                            self.quoted(&v)
                        }
                    };
                    let ty = self.pick(&[
                        "CDATA",
                        "ID",
                        "IDREF",
                        "NMTOKEN",
                        "NMTOKENS",
                        "ENTITY",
                        "(x|y)",
                        "NOTATION (n)",
                    ]);
                    format!("<!ATTLIST {} {} {ty} {def}>", self.name(), self.name())
                }
                4 | 5 => {
                    let n = self.pick(&["e1", "e2", "pe", "ext", "amp", "lt"]);
                    let v = match self.below(4) {
                        0 => format!("SYSTEM {}", self.quoted("ext.xml")),
                        1 => format!(
                            "PUBLIC {} {}",
                            self.quoted("-//X//Y"),
                            self.quoted("ext.xml")
                        ),
                        _ => {
                            let v = self.pick(&[
                                "value",
                                "&#60;b/&#62;",
                                "a&e1;b",
                                "&#38;#38;",
                                "<c>t</c>",
                                "x%pe;",
                                "&#65;",
                            ]);
                            self.quoted(v)
                        }
                    };
                    format!("<!ENTITY {n} {v}>")
                }
                6 => {
                    let v = self.pick(&["<!ELEMENT q ANY>", "x", "<!ENTITY e1 'p'>"]);
                    format!("<!ENTITY % pe {}>", self.quoted(v))
                }
                7 => "%pe;".into(),
                8 => format!("<!NOTATION n SYSTEM {}>", self.quoted("n")),
                9 => format!("<!--{}-->", self.pick(TEXT)),
                _ => format!("<?p {}?>", self.pick(TEXT)),
            };
            s.push_str(&item);
        }
        s.push_str(self.ws());
        s
    }

    fn attr_value_plain(&mut self) -> String {
        self.pick(&["d", "1 2", "a&amp;b", "&#65;", "", "x\ty", "&e1;", "<"])
            .into()
    }

    /// A document as UTF-8 text, and the encoding it is to be written in.
    pub fn document(&mut self) -> (String, &'static str) {
        let mut s = String::new();
        let enc = match if self.simple { 99 } else { self.below(12) } {
            0 => "ISO-8859-1",
            1 => "UTF-16",
            2 => "US-ASCII",
            3 => "utf-8",
            4 => "UTF-16LE?",
            5 => "UTF-16BE?",
            6 => "latin1-undeclared",
            _ => "UTF-8",
        };
        let bom8 = enc == "UTF-8" && self.chance(6);
        if bom8 {
            s.push('\u{feff}');
        }
        if self.chance(5) {
            s.push_str(self.ws());
        }
        let decl_enc = match enc {
            "UTF-16LE?" | "UTF-16BE?" => self.pick(&["UTF-16", "", "utf-16"]),
            "latin1-undeclared" => "",
            e => e,
        };
        if !self.chance(5) || !decl_enc.is_empty() {
            let mut d = String::from("<?xml");
            match self.below(12) {
                0 => {}
                1 => d.push_str(" version='1.1'"),
                2 => d.push_str(" version=\"1.0"),
                _ => d.push_str(&format!(" version{}={}\"1.0\"", self.ws(), self.ws())),
            }
            if !decl_enc.is_empty() {
                d.push_str(&format!(" encoding={}", self.quoted(decl_enc)));
            } else if self.chance(10) {
                d.push_str(&format!(
                    " encoding={}",
                    self.qpick(&["foo", "UTF-8x", "8859-1", "ascii", "iso-8859-1"])
                ));
            }
            if self.chance(4) {
                d.push_str(&format!(
                    " standalone={}",
                    self.qpick(&["yes", "no", "maybe"])
                ));
            }
            if self.chance(25) {
                d.push_str(" x='1'");
            }
            d.push_str(self.ws());
            d.push_str("?>");
            s.push_str(&d);
        }
        for _ in 0..self.below(3) {
            s.push_str(self.ws());
            match self.below(3) {
                0 => s.push_str(&format!("<!--{}-->", self.pick(TEXT))),
                1 => s.push_str(&format!(
                    "<?{} {}?>",
                    self.pick(&["p", "xml", "xml-stylesheet"]),
                    self.pick(TEXT)
                )),
                _ => {}
            }
        }
        if self.chance(2) {
            s.push_str(self.ws());
            let name = self.name();
            let ext = match self.below(4) {
                0 => String::new(),
                1 => format!(
                    " SYSTEM {}",
                    self.quoted("http://gifti.projects.nitrc.org/gifti.dtd")
                ),
                2 => format!(
                    " PUBLIC {} {}",
                    self.qpick(&["-//A//B", "bad\u{7}id", "x y"]),
                    self.quoted("s.dtd")
                ),
                _ => format!(" SYSTEM {}", self.quoted("")),
            };
            let sub = if !self.simple && self.chance(2) {
                format!(" [{}]", self.internal_subset())
            } else {
                String::new()
            };
            s.push_str(&format!(
                "<!DOCTYPE {name}{ext}{}{sub}{}>",
                self.ws(),
                self.ws()
            ));
        }
        s.push_str(self.ws());
        if !self.chance(40) {
            self.element(0, &mut s);
        }
        for _ in 0..self.below(3) {
            s.push_str(self.ws());
            match self.below(5) {
                0 => s.push_str(&format!("<!--{}-->", self.pick(TEXT))),
                1 => s.push_str("<?p d?>"),
                2 => s.push_str("<x/>"),
                3 => s.push_str("junk"),
                _ => {}
            }
        }
        (s, enc)
    }

    /// The document's bytes in its encoding (characters a single-byte encoding cannot hold
    /// become '?'), then mutated or cut now and then.
    pub fn bytes(&mut self) -> Vec<u8> {
        let (s, enc) = self.document();
        let mut b: Vec<u8> = match enc {
            "ISO-8859-1" | "latin1-undeclared" => s
                .chars()
                .map(|c| {
                    if (c as u32) < 256 {
                        c as u32 as u8
                    } else {
                        b'?'
                    }
                })
                .collect(),
            "US-ASCII" => s
                .chars()
                .map(|c| if (c as u32) < 128 { c as u8 } else { b'?' })
                .collect(),
            "UTF-16" | "UTF-16LE?" | "UTF-16BE?" => {
                let le = enc == "UTF-16LE?" || (enc == "UTF-16" && self.chance(2));
                let bom = enc == "UTF-16" || self.chance(3);
                let mut v = Vec::new();
                let units: Vec<u16> = s.encode_utf16().collect();
                if bom {
                    v.extend_from_slice(if le { &[0xFF, 0xFE] } else { &[0xFE, 0xFF] });
                }
                for u in units {
                    v.extend_from_slice(&if le { u.to_le_bytes() } else { u.to_be_bytes() });
                }
                v
            }
            _ => s.into_bytes(),
        };
        match self.below(10) {
            0 if !b.is_empty() => {
                let n = self.below(b.len() as u64) as usize;
                b.truncate(n);
            }
            1 if !b.is_empty() => {
                for _ in 0..1 + self.below(3) {
                    let at = self.below(b.len() as u64) as usize;
                    b[at] = [
                        b'<', b'>', b'&', 0, 0xff, 0xc3, 0x80, b']', b'\r', b'"', b'\'', b'=',
                        b'/', b'?', b'!', b'-', b'#', b';', b'%', b'[',
                    ][self.below(20) as usize];
                }
            }
            2 if !b.is_empty() => {
                let at = self.below(b.len() as u64) as usize;
                let n = (1 + self.below(8) as usize).min(b.len() - at);
                b.drain(at..at + n);
            }
            _ => {}
        }
        b
    }

    /// The sizes of the `XML_Parse` calls: gifticlib's (whole buffers, then a short final call),
    /// random small pieces, or one call.
    pub fn pieces(&mut self, len: usize) -> Vec<i32> {
        let mut v = Vec::new();
        match self.below(4) {
            0 => v.push(len as i32),
            1 => {
                let bs = [1usize, 2, 3, 7, 64, 1023, 1024, 1025, 2048][self.below(9) as usize];
                let mut left = len;
                while left >= bs {
                    v.push(bs as i32);
                    left -= bs;
                }
                v.push(left as i32);
            }
            _ => {
                let mut left = len;
                while left > 0 {
                    let n = (1 + self.below(40) as usize).min(left);
                    v.push(n as i32);
                    left -= n;
                }
                if self.chance(2) {
                    v.push(0);
                }
            }
        }
        if v.is_empty() {
            v.push(0);
        }
        v
    }
}
