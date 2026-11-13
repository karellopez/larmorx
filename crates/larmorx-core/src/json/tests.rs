// SPDX-License-Identifier: Apache-2.0
//! Unit cases of the JSON reader and writer (the expectations are nlohmann/json 3.2.0's, compiled
//! by GCC 8.4.0; the probe `validation/probes/fs-json` compares every path on generated
//! documents), and digests recorded on Linux x86-64 so every platform is checked against it.

use super::*;
use crate::defects::{Notices, UpstreamDefects};

fn err_text(d: &[u8]) -> String {
    String::from_utf8_lossy(&parse(d, true).unwrap_err().message).into_owned()
}

#[test]
fn parse_errors_as_nlohmann() {
    let cases: [(&[u8], &str); 8] = [
        (
            b"[1, tru]",
            "[json.exception.parse_error.101] parse error at 8: syntax error - invalid literal; last read: '1, tru]'",
        ),
        (
            b"{\"a\":1 \"b\"}",
            "[json.exception.parse_error.101] parse error at 10: syntax error - unexpected string literal; expected '}'",
        ),
        (
            b"",
            "[json.exception.parse_error.101] parse error at 1: syntax error - unexpected end of input; expected '[', '{', or a literal",
        ),
        (
            b"[1,]",
            "[json.exception.parse_error.101] parse error at 4: syntax error - unexpected ']'; expected '[', '{', or a literal",
        ),
        (
            b"\"\\ud800x\"",
            "[json.exception.parse_error.101] parse error at 8: syntax error - invalid string: surrogate U+DC00..U+DFFF must be followed by U+DC00..U+DFFF; last read: '\"\\ud800x'",
        ),
        (
            b"1e400",
            "[json.exception.out_of_range.406] number overflow parsing '1e400'",
        ),
        (
            b"{\"k\":\"\x01\"}",
            "[json.exception.parse_error.101] parse error at 7: syntax error - invalid string: control character must be escaped; last read: '\"<U+0001>'",
        ),
        (
            b"[-]",
            "[json.exception.parse_error.101] parse error at 3: syntax error - invalid number; expected digit after '-'; last read: '-]'",
        ),
    ];
    for (d, want) in cases {
        assert_eq!(err_text(d), want, "{:?}", String::from_utf8_lossy(d));
    }
    assert_eq!(parse(b"\xef\xbb", true).unwrap_err().message, b"[json.exception.parse_error.101] parse error at 3: syntax error - invalid BOM; must be 0xEF 0xBB 0xBF if given; last read: '\xef\xbb'");
}

#[test]
fn values_and_streams() {
    let (v, _) = parse(
        b"{\"a\":1,\"a\":-2,\"b\":18446744073709551616,\"c\":-0}",
        true,
    )
    .unwrap();
    let Value::Object(o) = &v else { panic!() };
    assert_eq!(o[&b"a"[..]], Value::Integer(-2));
    assert_eq!(o[&b"b"[..]], Value::Float(18446744073709551616.0));
    assert_eq!(o[&b"c"[..]], Value::Integer(0));
    // operator>>: what follows is left (a number's next character is taken)
    assert_eq!(parse(b"12 x", false).unwrap(), (Value::Unsigned(12), 3));
    assert_eq!(parse(b"[1] x", false).unwrap().1, 3);
    // a NUL byte ends the input
    assert_eq!(parse(b"1\x00garbage", true).unwrap().0, Value::Unsigned(1));
}

#[test]
fn numbers_as_to_chars() {
    let cases = [
        (0.1, "0.1"),
        (1e15, "1e+15"),
        (1e14, "100000000000000.0"),
        (1e16, "1e+16"),
        (0.0001, "0.0001"),
        (0.00001, "1e-05"),
        (5e-324, "5e-324"),
        (1.7976931348623157e308, "1.7976931348623157e+308"),
        (-0.0, "-0.0"),
        (123456789.125, "123456789.125"),
        (2.5e-5, "2.5e-05"),
        (1e21, "1e+21"),
    ];
    for (x, want) in cases {
        assert_eq!(to_chars(x), want);
    }
}

#[test]
fn writers_as_nlohmann() {
    let (v, _) = parse(b"{\"b\":[1,{\"c\":null}],\"a\":\"x\\u00e9\\u0001\"}", true).unwrap();
    let mut n = Notices::new(UpstreamDefects::Fix);
    assert_eq!(
        String::from_utf8(v.dump(2, b' ', false, &mut n).unwrap()).unwrap(),
        "{\n  \"a\": \"x\u{e9}\\u0001\",\n  \"b\": [\n    1,\n    {\n      \"c\": null\n    }\n  ]\n}"
    );
    assert_eq!(
        String::from_utf8(v.dump(-1, b' ', true, &mut n).unwrap()).unwrap(),
        "{\"a\":\"x\\u00e9\\u0001\",\"b\":[1,{\"c\":null}]}"
    );
    let (s, r) = v.write_stream(3, b' ', &mut n);
    assert!(r.is_ok());
    assert!(String::from_utf8(s).unwrap().starts_with("{\n   \"a\": "));
    let bad = Value::String(b"\xc3\xa9\xff".to_vec());
    assert_eq!(
        bad.dump(-1, b' ', false, &mut n).unwrap_err().message,
        b"[json.exception.type_error.316] invalid UTF-8 byte at index 2: 0xFF"
    );
    // a stream keeps what was written before the exception: here the string's quote, not the
    // bytes still in the escape buffer
    let mut obj = parse(b"{\"a\":\"x\"}", true).unwrap().0;
    *obj.index_key(b"a", &Nlohmann).unwrap() = bad.clone();
    let (s, r) = obj.write_stream(0, b' ', &mut n);
    assert!(r.is_err());
    assert_eq!(s, b"{\"a\":\"");
    assert!(n.is_empty());
}

#[test]
fn accessors_as_basic_json() {
    let mut v = parse(b"{\"points\":[{\"x\":1.5},{\"x\":true}]}", true)
        .unwrap()
        .0;
    let d = &Nlohmann;
    let p = v.index_key(b"points", d).unwrap();
    assert_eq!(p.size(), 2);
    let x0 = p
        .index(0, d)
        .unwrap()
        .index_key(b"x", d)
        .unwrap()
        .get_f64()
        .unwrap();
    assert_eq!(x0, 1.5);
    let x1 = p.index(1, d).unwrap().index_key(b"x", d).unwrap();
    assert_eq!(x1.get_f32().unwrap(), 1.0);
    assert_eq!(
        x1.get_f64().unwrap_err().message,
        b"[json.exception.type_error.302] type must be number, but is boolean"
    );
    assert_eq!(
        x1.index_key(b"y", d).unwrap_err().message,
        b"[json.exception.type_error.305] cannot use operator[] with boolean"
    );
    // a missing key is added as null; an index past the end grows the array
    let mut w = Value::Null;
    assert_eq!(w.index(2, d).unwrap(), &Value::Null);
    assert_eq!(w.size(), 3);
    assert_eq!(Value::Unsigned(7).iter().count(), 1);
    assert_eq!(Value::Null.iter().count(), 0);
}

#[test]
fn indent_growth_fs99() {
    let mut deep = b"[".repeat(130);
    deep.extend_from_slice(b"1");
    deep.extend_from_slice(&b"]".repeat(130));
    let v = parse(&deep, true).unwrap().0;
    let mut fix = Notices::new(UpstreamDefects::Fix);
    let a = v.dump(4, b'\t', false, &mut fix).unwrap();
    let mut rep = Notices::new(UpstreamDefects::Reproduce);
    let b = v.dump(4, b'\t', false, &mut rep).unwrap();
    assert!(fix.contains(&defects::FS_99) && rep.contains(&defects::FS_99));
    assert!(!a.windows(2).any(|w| w == b"\t ") && b.windows(2).any(|w| w == b"\t "));
    // nested past what the original's recursion survives: written, named
    let mut very = b"[".repeat(DEEP_NESTING + 5);
    very.extend_from_slice(&b"]".repeat(DEEP_NESTING + 5));
    let v = parse(&very, true).unwrap().0;
    let mut n = Notices::new(UpstreamDefects::Fix);
    assert_eq!(v.dump(-1, b' ', false, &mut n).unwrap(), very);
    assert!(n.contains(&defects::FS_99));
    // a million levels parse and drop without recursion
    let mut huge = b"[".repeat(1_000_000);
    huge.extend_from_slice(&b"]".repeat(1_000_000));
    assert_eq!(parse(&huge, true).unwrap().0.size(), 1);
}

struct Fnv(u64);
impl Fnv {
    fn eat(&mut self, b: &[u8]) {
        for &x in b {
            self.0 = (self.0 ^ x as u64).wrapping_mul(0x100_0000_01b3);
        }
    }
}

#[test]
fn digest_of_doubles() {
    let mut s = 0x2026_1113u64;
    let mut h = Fnv(0xcbf2_9ce4_8422_2325);
    for i in 0..200_000u64 {
        s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let bits = if i % 2 == 0 { s } else { s >> (i % 64) };
        let x = f64::from_bits(bits);
        if x.is_finite() {
            h.eat(to_chars(x).as_bytes());
            h.eat(b"\n");
        }
    }
    assert_eq!(
        h.0, 0x4e09_3f2a_970d_03f1,
        "digest of the doubles: {:#018x}",
        h.0
    );
}
