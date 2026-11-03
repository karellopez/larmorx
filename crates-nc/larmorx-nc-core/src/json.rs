// SPDX-License-Identifier: LicenseRef-FSL AND MIT
// Translated to Rust from nlohmann/json 3.12.0 as FSL 6.0.7.23 ships it (`include/nlohmann/`:
// detail/conversions/to_chars.hpp, detail/output/serializer.hpp's dump, dump_float and
// dump_integer), Copyright (c) 2013-2025 Niels Lohmann, SPDX-License-Identifier: MIT; its Grisu2
// is Florian Loitsch's (Copyright (c) 2009 Florian Loitsch, MIT). The MIT licence:
//
//   Permission is hereby granted, free of charge, to any person obtaining a copy of this software
//   and associated documentation files (the "Software"), to deal in the Software without
//   restriction, including without limitation the rights to use, copy, modify, merge, publish,
//   distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the
//   Software is furnished to do so, subject to the following conditions: The above copyright
//   notice and this permission notice shall be included in all copies or substantial portions of
//   the Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//   IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
//   PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
//   LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
//   OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
//   DEALINGS IN THE SOFTWARE.
//
// Rust translation Copyright 2026 Karel Lopez Vilaret.
//! The JSON that FSL's programs print through nlohmann/json (`fslstats -json`): values,
//! objects with their keys in sorted order (`std::map`), and `dump(4)`'s text. Numbers are
//! written as nlohmann writes them: integers in decimal, floating-point values (`double`) by
//! Grisu2 ([`to_chars`]), which gives a short decimal that reads back to the same value but is
//! not always the shortest or the nearest one, and `null` for NaN and infinities.

use std::collections::BTreeMap;

/// A JSON value as nlohmann stores one (`number_integer`, `number_float`, arrays, objects).
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Array(Vec<Value>),
    Object(BTreeMap<String, Value>),
}

impl Value {
    /// `json::object()`.
    pub fn object() -> Value {
        Value::Object(BTreeMap::new())
    }

    /// `json::array()`.
    pub fn array() -> Value {
        Value::Array(Vec::new())
    }

    /// `j[key] = value` on an object.
    pub fn set(&mut self, key: &str, value: Value) {
        if let Value::Object(m) = self {
            m.insert(key.to_owned(), value);
        }
    }

    /// `j += value` on an array.
    pub fn push(&mut self, value: Value) {
        if let Value::Array(a) = self {
            a.push(value);
        }
    }

    /// `j[key]` of an object (inserted as null-free `[]` when missing is not needed here).
    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        match self {
            Value::Object(m) => m.get_mut(key),
            _ => None,
        }
    }

    /// An array of floating-point values.
    pub fn floats(v: &[f64]) -> Value {
        Value::Array(v.iter().map(|&x| Value::Float(x)).collect())
    }

    /// An array of integers.
    pub fn ints(v: &[i64]) -> Value {
        Value::Array(v.iter().map(|&x| Value::Int(x)).collect())
    }

    /// `dump(indent)` with `indent >= 0` (pretty printed).
    pub fn dump(&self, indent: usize) -> String {
        let mut s = String::new();
        dump_into(self, indent, 0, &mut s);
        s
    }
}

fn dump_into(v: &Value, step: usize, current: usize, o: &mut String) {
    match v {
        Value::Object(m) => {
            if m.is_empty() {
                o.push_str("{}");
                return;
            }
            o.push_str("{\n");
            let new_indent = current + step;
            let n = m.len();
            for (k, (key, val)) in m.iter().enumerate() {
                push_spaces(o, new_indent);
                o.push('"');
                o.push_str(&escape(key));
                o.push_str("\": ");
                dump_into(val, step, new_indent, o);
                if k + 1 < n {
                    o.push_str(",\n");
                } else {
                    o.push('\n');
                }
            }
            push_spaces(o, current);
            o.push('}');
        }
        Value::Array(a) => {
            if a.is_empty() {
                o.push_str("[]");
                return;
            }
            o.push_str("[\n");
            let new_indent = current + step;
            let n = a.len();
            for (k, val) in a.iter().enumerate() {
                push_spaces(o, new_indent);
                dump_into(val, step, new_indent, o);
                if k + 1 < n {
                    o.push_str(",\n");
                } else {
                    o.push('\n');
                }
            }
            push_spaces(o, current);
            o.push(']');
        }
        Value::Int(i) => o.push_str(&i.to_string()),
        Value::Float(x) => {
            if !x.is_finite() {
                o.push_str("null");
            } else {
                o.push_str(&to_chars(*x));
            }
        }
    }
}

fn push_spaces(o: &mut String, n: usize) {
    o.extend(std::iter::repeat_n(' ', n));
}

/// `dump_escaped` for the ASCII keys FSL's programs use (quotes, backslashes and control
/// characters escaped as nlohmann escapes them).
fn escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

// ------------------------------------------------------------------------------------------------
// Grisu2 (dtoa_impl)

#[derive(Clone, Copy)]
struct DiyFp {
    f: u64,
    e: i32,
}

impl DiyFp {
    fn sub(x: DiyFp, y: DiyFp) -> DiyFp {
        DiyFp {
            f: x.f - y.f,
            e: x.e,
        }
    }

    fn mul(x: DiyFp, y: DiyFp) -> DiyFp {
        let u_lo = x.f & 0xFFFF_FFFF;
        let u_hi = x.f >> 32;
        let v_lo = y.f & 0xFFFF_FFFF;
        let v_hi = y.f >> 32;
        let p0 = u_lo * v_lo;
        let p1 = u_lo * v_hi;
        let p2 = u_hi * v_lo;
        let p3 = u_hi * v_hi;
        let p0_hi = p0 >> 32;
        let p1_lo = p1 & 0xFFFF_FFFF;
        let p1_hi = p1 >> 32;
        let p2_lo = p2 & 0xFFFF_FFFF;
        let p2_hi = p2 >> 32;
        let mut q = p0_hi + p1_lo + p2_lo;
        q += 1u64 << (64 - 32 - 1);
        let h = p3 + p2_hi + p1_hi + (q >> 32);
        DiyFp {
            f: h,
            e: x.e + y.e + 64,
        }
    }

    fn normalize(mut x: DiyFp) -> DiyFp {
        while (x.f >> 63) == 0 {
            x.f <<= 1;
            x.e -= 1;
        }
        x
    }

    fn normalize_to(x: DiyFp, target: i32) -> DiyFp {
        let delta = x.e - target;
        DiyFp {
            f: x.f << delta,
            e: target,
        }
    }
}

/// `compute_boundaries(double)`: `(w, minus, plus)`.
fn compute_boundaries(value: f64) -> (DiyFp, DiyFp, DiyFp) {
    const PRECISION: i32 = 53;
    const BIAS: i32 = 1024 - 1 + (PRECISION - 1);
    const MIN_EXP: i32 = 1 - BIAS;
    const HIDDEN: u64 = 1u64 << (PRECISION - 1);
    let bits = value.to_bits();
    let e = bits >> (PRECISION - 1);
    let f = bits & (HIDDEN - 1);
    let v = if e == 0 {
        DiyFp { f, e: MIN_EXP }
    } else {
        DiyFp {
            f: f + HIDDEN,
            e: e as i32 - BIAS,
        }
    };
    let lower_closer = f == 0 && e > 1;
    let m_plus = DiyFp {
        f: 2 * v.f + 1,
        e: v.e - 1,
    };
    let m_minus = if lower_closer {
        DiyFp {
            f: 4 * v.f - 1,
            e: v.e - 2,
        }
    } else {
        DiyFp {
            f: 2 * v.f - 1,
            e: v.e - 1,
        }
    };
    let w_plus = DiyFp::normalize(m_plus);
    let w_minus = DiyFp::normalize_to(m_minus, w_plus.e);
    (DiyFp::normalize(v), w_minus, w_plus)
}

const ALPHA: i32 = -60;

#[rustfmt::skip]
const CACHED_POWERS: [(u64, i32, i32); 79] = [
    (0xAB70FE17C79AC6CA, -1060, -300), (0xFF77B1FCBEBCDC4F, -1034, -292),
    (0xBE5691EF416BD60C, -1007, -284), (0x8DD01FAD907FFC3C, -980, -276),
    (0xD3515C2831559A83, -954, -268), (0x9D71AC8FADA6C9B5, -927, -260),
    (0xEA9C227723EE8BCB, -901, -252), (0xAECC49914078536D, -874, -244),
    (0x823C12795DB6CE57, -847, -236), (0xC21094364DFB5637, -821, -228),
    (0x9096EA6F3848984F, -794, -220), (0xD77485CB25823AC7, -768, -212),
    (0xA086CFCD97BF97F4, -741, -204), (0xEF340A98172AACE5, -715, -196),
    (0xB23867FB2A35B28E, -688, -188), (0x84C8D4DFD2C63F3B, -661, -180),
    (0xC5DD44271AD3CDBA, -635, -172), (0x936B9FCEBB25C996, -608, -164),
    (0xDBAC6C247D62A584, -582, -156), (0xA3AB66580D5FDAF6, -555, -148),
    (0xF3E2F893DEC3F126, -529, -140), (0xB5B5ADA8AAFF80B8, -502, -132),
    (0x87625F056C7C4A8B, -475, -124), (0xC9BCFF6034C13053, -449, -116),
    (0x964E858C91BA2655, -422, -108), (0xDFF9772470297EBD, -396, -100),
    (0xA6DFBD9FB8E5B88F, -369, -92), (0xF8A95FCF88747D94, -343, -84),
    (0xB94470938FA89BCF, -316, -76), (0x8A08F0F8BF0F156B, -289, -68),
    (0xCDB02555653131B6, -263, -60), (0x993FE2C6D07B7FAC, -236, -52),
    (0xE45C10C42A2B3B06, -210, -44), (0xAA242499697392D3, -183, -36),
    (0xFD87B5F28300CA0E, -157, -28), (0xBCE5086492111AEB, -130, -20),
    (0x8CBCCC096F5088CC, -103, -12), (0xD1B71758E219652C, -77, -4),
    (0x9C40000000000000, -50, 4), (0xE8D4A51000000000, -24, 12),
    (0xAD78EBC5AC620000, 3, 20), (0x813F3978F8940984, 30, 28),
    (0xC097CE7BC90715B3, 56, 36), (0x8F7E32CE7BEA5C70, 83, 44),
    (0xD5D238A4ABE98068, 109, 52), (0x9F4F2726179A2245, 136, 60),
    (0xED63A231D4C4FB27, 162, 68), (0xB0DE65388CC8ADA8, 189, 76),
    (0x83C7088E1AAB65DB, 216, 84), (0xC45D1DF942711D9A, 242, 92),
    (0x924D692CA61BE758, 269, 100), (0xDA01EE641A708DEA, 295, 108),
    (0xA26DA3999AEF774A, 322, 116), (0xF209787BB47D6B85, 348, 124),
    (0xB454E4A179DD1877, 375, 132), (0x865B86925B9BC5C2, 402, 140),
    (0xC83553C5C8965D3D, 428, 148), (0x952AB45CFA97A0B3, 455, 156),
    (0xDE469FBD99A05FE3, 481, 164), (0xA59BC234DB398C25, 508, 172),
    (0xF6C69A72A3989F5C, 534, 180), (0xB7DCBF5354E9BECE, 561, 188),
    (0x88FCF317F22241E2, 588, 196), (0xCC20CE9BD35C78A5, 614, 204),
    (0x98165AF37B2153DF, 641, 212), (0xE2A0B5DC971F303A, 667, 220),
    (0xA8D9D1535CE3B396, 694, 228), (0xFB9B7CD9A4A7443C, 720, 236),
    (0xBB764C4CA7A44410, 747, 244), (0x8BAB8EEFB6409C1A, 774, 252),
    (0xD01FEF10A657842C, 800, 260), (0x9B10A4E5E9913129, 827, 268),
    (0xE7109BFBA19C0C9D, 853, 276), (0xAC2820D9623BF429, 880, 284),
    (0x80444B5E7AA7CF85, 907, 292), (0xBF21E44003ACDD2D, 933, 300),
    (0x8E679C2F5E44FF8F, 960, 308), (0xD433179D9C8CB841, 986, 316),
    (0x9E19DB92B4E31BA9, 1013, 324),
];

/// `get_cached_power_for_binary_exponent(e)`: `(f, e, k)`.
fn cached_power(e: i32) -> (u64, i32, i32) {
    const MIN_DEC_EXP: i32 = -300;
    const DEC_STEP: i32 = 8;
    let f = ALPHA - e - 1;
    let k = (f * 78913) / (1 << 18) + i32::from(f > 0);
    let index = (-MIN_DEC_EXP + k + (DEC_STEP - 1)) / DEC_STEP;
    CACHED_POWERS[index as usize]
}

/// `find_largest_pow10(n, pow10)`: `(k, pow10)` with `10^(k-1) <= n < 10^k`.
fn find_largest_pow10(n: u32) -> (i32, u32) {
    let mut p = 1_000_000_000u32;
    let mut k = 10;
    while k > 1 && n < p {
        p /= 10;
        k -= 1;
    }
    (k, p)
}

fn grisu2_round(buf: &mut [u8], len: usize, dist: u64, delta: u64, mut rest: u64, ten_k: u64) {
    while rest < dist
        && delta - rest >= ten_k
        && (rest.wrapping_add(ten_k) < dist
            || dist - rest > rest.wrapping_add(ten_k).wrapping_sub(dist))
    {
        buf[len - 1] -= 1;
        rest += ten_k;
    }
}

fn grisu2_digit_gen(
    buf: &mut [u8],
    len: &mut usize,
    dec_exp: &mut i32,
    m_minus: DiyFp,
    w: DiyFp,
    m_plus: DiyFp,
) {
    let mut delta = DiyFp::sub(m_plus, m_minus).f;
    let mut dist = DiyFp::sub(m_plus, w).f;
    let one_e = m_plus.e;
    let one_f = 1u64 << (-one_e);
    let mut p1 = (m_plus.f >> (-one_e)) as u32;
    let mut p2 = m_plus.f & (one_f - 1);
    let (k, mut pow10) = find_largest_pow10(p1);
    let mut n = k;
    while n > 0 {
        let d = p1 / pow10;
        let r = p1 % pow10;
        buf[*len] = b'0' + d as u8;
        *len += 1;
        p1 = r;
        n -= 1;
        let rest = (u64::from(p1) << (-one_e)) + p2;
        if rest <= delta {
            *dec_exp += n;
            let ten_n = u64::from(pow10) << (-one_e);
            grisu2_round(buf, *len, dist, delta, rest, ten_n);
            return;
        }
        pow10 /= 10;
    }
    let mut m = 0;
    loop {
        p2 = p2.wrapping_mul(10);
        let d = p2 >> (-one_e);
        let r = p2 & (one_f - 1);
        buf[*len] = b'0' + d as u8;
        *len += 1;
        p2 = r;
        m += 1;
        delta = delta.wrapping_mul(10);
        dist = dist.wrapping_mul(10);
        if p2 <= delta {
            break;
        }
    }
    *dec_exp -= m;
    grisu2_round(buf, *len, dist, delta, p2, one_f);
}

/// `grisu2(buf, len, decimal_exponent, value)`: the digits and the decimal exponent.
fn grisu2(value: f64) -> (Vec<u8>, i32) {
    let (v, m_minus, m_plus) = compute_boundaries(value);
    let (cf, ce, ck) = cached_power(m_plus.e);
    let c = DiyFp { f: cf, e: ce };
    let w = DiyFp::mul(v, c);
    let w_minus = DiyFp::mul(m_minus, c);
    let w_plus = DiyFp::mul(m_plus, c);
    let mm = DiyFp {
        f: w_minus.f + 1,
        e: w_minus.e,
    };
    let mp = DiyFp {
        f: w_plus.f - 1,
        e: w_plus.e,
    };
    let mut buf = [0u8; 32];
    let mut len = 0usize;
    let mut dec_exp = -ck;
    grisu2_digit_gen(&mut buf, &mut len, &mut dec_exp, mm, w, mp);
    (buf[..len].to_vec(), dec_exp)
}

/// `format_buffer(buf, len, decimal_exponent, -4, 15)`.
fn format_buffer(digits: &[u8], dec_exp: i32, min_exp: i32, max_exp: i32) -> String {
    let k = digits.len() as i32;
    let n = k + dec_exp;
    let d = std::str::from_utf8(digits).unwrap_or("");
    if k <= n && n <= max_exp {
        // digits, zeros, ".0"
        return format!("{d}{}.0", "0".repeat((n - k) as usize));
    }
    if 0 < n && n <= max_exp {
        let (a, b) = d.split_at(n as usize);
        return format!("{a}.{b}");
    }
    if min_exp < n && n <= 0 {
        return format!("0.{}{d}", "0".repeat((-n) as usize));
    }
    let mut s = String::new();
    if k == 1 {
        s.push_str(d);
    } else {
        s.push_str(&d[..1]);
        s.push('.');
        s.push_str(&d[1..]);
    }
    s.push('e');
    let e = n - 1;
    s.push(if e < 0 { '-' } else { '+' });
    let e = e.unsigned_abs();
    if e < 10 {
        s.push('0');
    }
    s.push_str(&e.to_string());
    s
}

/// `to_chars(first, last, double)`: the sign, `0.0` for a zero, else Grisu2's digits in
/// fixed notation for decimal exponents in [-4, 15) and in exponential notation otherwise.
pub fn to_chars(value: f64) -> String {
    let mut out = String::new();
    let mut v = value;
    if v.is_sign_negative() {
        v = -v;
        out.push('-');
    }
    if v == 0.0 {
        out.push_str("0.0");
        return out;
    }
    let (digits, dec_exp) = grisu2(v);
    out.push_str(&format_buffer(&digits, dec_exp, -4, 15));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_as_nlohmann_writes_them() {
        assert_eq!(to_chars(0.0), "0.0");
        assert_eq!(to_chars(-0.0), "-0.0");
        assert_eq!(to_chars(2000.0), "2000.0");
        assert_eq!(to_chars(0.5266819608311916), "0.5266819608311916");
        assert_eq!(to_chars(f64::from(0.018357284f32)), "0.018357284367084503");
        assert_eq!(to_chars(1e20), "1e+20");
        assert_eq!(to_chars(1.5e-5), "1.5e-05");
        assert_eq!(to_chars(0.0001), "0.0001");
        assert_eq!(to_chars(123456789012345678.0), "1.2345678901234568e+17");
        assert_eq!(to_chars(f64::MIN_POSITIVE / 4.0), "5.562684646268003e-309");
        assert_eq!(to_chars(f64::MAX), "1.7976931348623157e+308");
        let mut o = Value::object();
        o.set("-x", Value::ints(&[4, 8]));
        o.set("-M", Value::Float(1.0));
        assert_eq!(
            o.dump(4),
            "{\n    \"-M\": 1.0,\n    \"-x\": [\n        4,\n        8\n    ]\n}"
        );
        assert_eq!(Value::array().dump(4), "[]");
    }
}
