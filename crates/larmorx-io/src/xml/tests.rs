// SPDX-License-Identifier: Apache-2.0
//! The parser's events on small documents, fed in pieces as gifticlib feeds it (expected values
//! from expat 2.0.1's behaviour, measured by validation/probes/fs-gifti).

use super::*;

#[derive(Default)]
struct Rec(Vec<String>);

fn s(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

impl Handler for Rec {
    fn start_element(&mut self, n: &[u8], a: &[(Vec<u8>, Vec<u8>)]) {
        let a: Vec<String> = a
            .iter()
            .map(|(k, v)| format!("{}={}", s(k), s(v)))
            .collect();
        self.0.push(format!("<{} {}>", s(n), a.join(" ")));
    }
    fn end_element(&mut self, n: &[u8]) {
        self.0.push(format!("</{}>", s(n)));
    }
    fn characters(&mut self, d: &[u8]) {
        self.0.push(format!("c[{}]", s(d)));
    }
    fn default(&mut self, d: &[u8]) {
        self.0.push(format!("d[{}]", s(d)));
    }
    fn comment(&mut self, d: &[u8]) {
        self.0.push(format!("!{}", s(d)));
    }
    fn xml_decl(&mut self, v: Option<&[u8]>, e: Option<&[u8]>, st: i32) {
        self.0
            .push(format!("xml {:?} {:?} {st}", v.map(s), e.map(s)));
    }
    fn element_decl(&mut self, n: &[u8], m: &Content) {
        self.0.push(format!(
            "el {} {} {} {}",
            s(n),
            m.kind,
            m.quant,
            m.children.len()
        ));
    }
}

fn run(pieces: &[&[u8]]) -> (Vec<String>, ErrorCode, u64) {
    let mut p = Parser::new();
    let mut r = Rec(Vec::new());
    for (k, piece) in pieces.iter().enumerate() {
        if p.parse(piece, k + 1 == pieces.len(), &mut r).is_err() {
            let line = p.current_line_number();
            return (r.0, p.error_code(), line);
        }
    }
    (r.0, ERROR_NONE, 0)
}

#[test]
fn chunks_and_events() {
    let (ev, e, _) = run(&[
        b"<?xml version=\"1.0\"?>\n<a x='1 &amp; 2'>t",
        b"ext\r\n]]x</a>\n",
        b"",
    ]);
    assert_eq!(e, ERROR_NONE);
    assert_eq!(
        ev,
        [
            "xml Some(\"1.0\") None -1",
            "d[\n]",
            "<a x=1 & 2>",
            "c[t]",
            "c[ext]",
            "c[\n]",
            "c[]]x]",
            "</a>",
            "d[\n]",
        ]
    );
}

#[test]
fn internal_subset_defaults_and_entities() {
    let doc = b"<!DOCTYPE a [<!ENTITY e 'E'><!ATTLIST a k CDATA 'v&e;'><!ELEMENT a (#PCDATA)*>]><a>&e;</a>";
    let (ev, e, _) = run(&[doc, b""]);
    assert_eq!(e, ERROR_NONE);
    // the declared default reaches the start tag with its entity expanded; a reference in
    // content goes to the default handler (internal entities are not expanded there)
    assert!(ev.iter().any(|x| x == "<a k=vE>"), "{ev:?}");
    assert!(ev.iter().any(|x| x == "el a 3 2 0"), "{ev:?}");
    assert!(ev.iter().any(|x| x == "d[&e;]"), "{ev:?}");
    // a reference to an entity not yet declared is an error in a default value
    let doc = b"<!DOCTYPE a [<!ATTLIST a k CDATA 'v&e;'><!ENTITY e 'E'>]><a/>";
    assert_eq!(run(&[doc, b""]).1, ERROR_UNDEFINED_ENTITY);
}

#[test]
fn errors_and_lines() {
    assert_eq!(run(&[b"<a>\n<b></a>", b""]).1, ERROR_TAG_MISMATCH);
    assert_eq!(run(&[b"<a>\n<b></a>", b""]).2, 2);
    assert_eq!(run(&[b"<a/><b/>", b""]).1, ERROR_JUNK_AFTER_DOC_ELEMENT);
    assert_eq!(run(&[b"<a>", b""]).1, ERROR_NO_ELEMENTS);
    assert_eq!(
        run(&[b"<?xml version='1.0' encoding='foo'?><a/>", b""]).1,
        ERROR_UNKNOWN_ENCODING
    );
    assert_eq!(
        run(&[b"\n\n<?xml version='1.0'?><a/>", b""]).1,
        ERROR_MISPLACED_XML_PI
    );
    assert_eq!(
        run(&[b"<a b='1' b='2'/>", b""]).1,
        ERROR_DUPLICATE_ATTRIBUTE
    );
}

#[test]
fn utf16_and_latin1_reach_handlers_as_utf8() {
    let text = "<a>\u{e9}t\u{e9}</a>";
    let mut le = vec![0xFF, 0xFE];
    for u in text.encode_utf16() {
        le.extend_from_slice(&u.to_le_bytes());
    }
    let (ev, e, _) = run(&[&le, b""]);
    assert_eq!(e, ERROR_NONE);
    assert_eq!(ev, ["<a >", "c[\u{e9}t\u{e9}]", "</a>"]);
    let latin = b"<?xml version='1.0' encoding='ISO-8859-1'?><a>\xe9</a>";
    let (ev, e, _) = run(&[latin, b""]);
    assert_eq!(e, ERROR_NONE);
    assert_eq!(ev[2], "c[\u{e9}]");
}
