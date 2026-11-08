// SPDX-License-Identifier: Apache-2.0 AND PSF-2.0
//! `json.dump(obj, fp, indent=N, sort_keys=...)` as CPython 3.12.16 writes it (`Lib/json/
//! encoder.py`, the pure-Python `_make_iterencode` that an `indent` selects): `ensure_ascii`
//! escapes, `", "`/`": "` separators with the item separator `","` when indented, nested
//! containers on their own lines, empty containers as `{}` and `[]`, floats by `float.__repr__`
//! (`NaN`, `Infinity`, `-Infinity` for the special values), and no newline at the end.

use super::number::float_repr;

/// A JSON value as Python's encoder sees it (a `dict` keeps its insertion order).
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i128),
    Float(f64),
    Str(String),
    List(Vec<Value>),
    Dict(Vec<(String, Value)>),
}

impl Value {
    /// A string value.
    pub fn str(s: impl Into<String>) -> Value {
        Value::Str(s.into())
    }
}

/// `json.encoder.py_encode_basestring_ascii(s)`: the string in double quotes, with `"`, `\`,
/// the control characters and everything outside `' '..='~'` escaped (`\uXXXX`, lower-case
/// hexadecimal, surrogate pairs above U+FFFF).
pub fn encode_str_ascii(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            ' '..='~' => out.push(c),
            _ => {
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{unit:04x}"));
                }
            }
        }
    }
    out.push('"');
    out
}

fn float_str(x: f64) -> String {
    if x.is_nan() {
        "NaN".to_owned()
    } else if x == f64::INFINITY {
        "Infinity".to_owned()
    } else if x == f64::NEG_INFINITY {
        "-Infinity".to_owned()
    } else {
        float_repr(x)
    }
}

fn encode(v: &Value, indent: usize, level: usize, sort_keys: bool, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Int(i) => out.push_str(&i.to_string()),
        Value::Float(x) => out.push_str(&float_str(*x)),
        Value::Str(s) => out.push_str(&encode_str_ascii(s)),
        Value::List(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            let inner = " ".repeat(indent * (level + 1));
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                out.push_str(if i == 0 { "\n" } else { ",\n" });
                out.push_str(&inner);
                encode(item, indent, level + 1, sort_keys, out);
            }
            out.push('\n');
            out.push_str(&" ".repeat(indent * level));
            out.push(']');
        }
        Value::Dict(items) => {
            if items.is_empty() {
                out.push_str("{}");
                return;
            }
            let mut order: Vec<&(String, Value)> = items.iter().collect();
            if sort_keys {
                // Python sorts `str` keys by code point, as Rust orders `String`s.
                order.sort_by(|a, b| a.0.cmp(&b.0));
            }
            let inner = " ".repeat(indent * (level + 1));
            out.push('{');
            for (i, (k, item)) in order.into_iter().enumerate() {
                out.push_str(if i == 0 { "\n" } else { ",\n" });
                out.push_str(&inner);
                out.push_str(&encode_str_ascii(k));
                out.push_str(": ");
                encode(item, indent, level + 1, sort_keys, out);
            }
            out.push('\n');
            out.push_str(&" ".repeat(indent * level));
            out.push('}');
        }
    }
}

/// `json.dumps(v, indent=indent, sort_keys=sort_keys)` (what `json.dump` writes).
pub fn dumps_indent(v: &Value, indent: usize, sort_keys: bool) -> String {
    let mut out = String::new();
    encode(v, indent, 0, sort_keys, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_python_writes_it() {
        let v = Value::Dict(vec![
            (
                "b".into(),
                Value::List(vec![Value::Int(1), Value::str("x\"\u{e9}")]),
            ),
            ("a".into(), Value::Dict(vec![])),
            ("c".into(), Value::Float(0.1)),
            ("d".into(), Value::List(vec![])),
        ]);
        assert_eq!(
            dumps_indent(&v, 4, true),
            "{\n    \"a\": {},\n    \"b\": [\n        1,\n        \"x\\\"\\u00e9\"\n    ],\n    \
             \"c\": 0.1,\n    \"d\": []\n}"
        );
        assert_eq!(
            encode_str_ascii("\u{1f600}\u{7f}\t"),
            "\"\\ud83d\\ude00\\u007f\\t\""
        );
    }
}
