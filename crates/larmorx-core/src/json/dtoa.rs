// SPDX-License-Identifier: Apache-2.0 AND MIT
//! nlohmann/json 3.2.0's `to_chars` for `double`: Grisu2 (Florian Loitsch's, as nlohmann wrote it:
//! `diyfp`, `compute_boundaries`, the cached powers, `grisu2_digit_gen`, `grisu2_round`) and
//! `format_buffer` (decimal notation for decimal exponents in (-4, 15], otherwise `d.ddde+XX`).
//! Grisu2 does not always give the shortest digits; these are its digits.

use super::tables::CACHED_POWERS;

#[derive(Clone, Copy)]
struct DiyFp {
    f: u64,
    e: i32,
}

impl DiyFp {
    fn sub(x: DiyFp, y: DiyFp) -> DiyFp {
        DiyFp {
            f: x.f.wrapping_sub(y.f),
            e: x.e,
        }
    }
    fn mul(x: DiyFp, y: DiyFp) -> DiyFp {
        let (u_lo, u_hi) = (x.f & 0xffff_ffff, x.f >> 32);
        let (v_lo, v_hi) = (y.f & 0xffff_ffff, y.f >> 32);
        let p0 = u_lo * v_lo;
        let p1 = u_lo * v_hi;
        let p2 = u_hi * v_lo;
        let p3 = u_hi * v_hi;
        let p0_hi = p0 >> 32;
        let (p1_lo, p1_hi) = (p1 & 0xffff_ffff, p1 >> 32);
        let (p2_lo, p2_hi) = (p2 & 0xffff_ffff, p2 >> 32);
        let mut q = p0_hi + p1_lo + p2_lo;
        q += 1u64 << 31;
        let h = p3 + p2_hi + p1_hi + (q >> 32);
        DiyFp {
            f: h,
            e: x.e + y.e + 64,
        }
    }
    fn normalize(mut x: DiyFp) -> DiyFp {
        while x.f >> 63 == 0 {
            x.f <<= 1;
            x.e -= 1;
        }
        x
    }
    fn normalize_to(x: DiyFp, target: i32) -> DiyFp {
        DiyFp {
            f: x.f << (x.e - target),
            e: target,
        }
    }
}

/// `compute_boundaries`: (w, m-, m+).
fn compute_boundaries(value: f64) -> (DiyFp, DiyFp, DiyFp) {
    const PRECISION: i32 = 53;
    const BIAS: i32 = 1024 - 1 + (PRECISION - 1);
    const MIN_EXP: i32 = 1 - BIAS;
    const HIDDEN: u64 = 1 << (PRECISION - 1);
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

fn cached_power(e: i32) -> (u64, i32, i32) {
    let f = ALPHA - e - 1;
    let k = (f * 78913) / (1 << 18) + (f > 0) as i32;
    let index = (300 + k + 7) / 8;
    CACHED_POWERS[index as usize]
}

fn find_largest_pow10(n: u32) -> (i32, u32) {
    const P: [u32; 10] = [
        1_000_000_000,
        100_000_000,
        10_000_000,
        1_000_000,
        100_000,
        10_000,
        1_000,
        100,
        10,
        1,
    ];
    for (i, &p) in P.iter().enumerate() {
        if n >= p || p == 1 {
            return (10 - i as i32, p);
        }
    }
    (1, 1)
}

fn grisu2_round(buf: &mut [u8], len: usize, dist: u64, delta: u64, mut rest: u64, ten_k: u64) {
    // C's unsigned arithmetic, wrapping as it does
    while rest < dist
        && delta.wrapping_sub(rest) >= ten_k
        && (rest.wrapping_add(ten_k) < dist
            || dist.wrapping_sub(rest) > rest.wrapping_add(ten_k).wrapping_sub(dist))
    {
        buf[len - 1] = buf[len - 1].wrapping_sub(1);
        rest = rest.wrapping_add(ten_k);
    }
}

fn grisu2_digit_gen(
    buf: &mut [u8],
    len: &mut usize,
    dexp: &mut i32,
    m_minus: DiyFp,
    w: DiyFp,
    m_plus: DiyFp,
) {
    let mut delta = DiyFp::sub(m_plus, m_minus).f;
    let mut dist = DiyFp::sub(m_plus, w).f;
    let one = DiyFp {
        f: 1u64 << -m_plus.e,
        e: m_plus.e,
    };
    let mut p1 = (m_plus.f >> -one.e) as u32;
    let mut p2 = m_plus.f & (one.f - 1);
    let (k, mut pow10) = find_largest_pow10(p1);
    let mut n = k;
    while n > 0 {
        let d = p1 / pow10;
        let r = p1 % pow10;
        buf[*len] = b'0' + d as u8;
        *len += 1;
        p1 = r;
        n -= 1;
        let rest = ((p1 as u64) << -one.e) + p2;
        if rest <= delta {
            *dexp += n;
            let ten_n = (pow10 as u64) << -one.e;
            grisu2_round(buf, *len, dist, delta, rest, ten_n);
            return;
        }
        pow10 /= 10;
    }
    let mut m = 0;
    loop {
        p2 = p2.wrapping_mul(10);
        let d = p2 >> -one.e;
        let r = p2 & (one.f - 1);
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
    *dexp -= m;
    grisu2_round(buf, *len, dist, delta, p2, one.f);
}

fn grisu2(buf: &mut [u8], len: &mut usize, dexp: &mut i32, value: f64) {
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
    *dexp = -ck;
    grisu2_digit_gen(buf, len, dexp, mm, w, mp);
}

fn append_exponent(out: &mut Vec<u8>, mut e: i32) {
    if e < 0 {
        e = -e;
        out.push(b'-');
    } else {
        out.push(b'+');
    }
    let k = e as u32;
    if k < 10 {
        out.push(b'0');
        out.push(b'0' + k as u8);
    } else if k < 100 {
        out.push(b'0' + (k / 10) as u8);
        out.push(b'0' + (k % 10) as u8);
    } else {
        out.push(b'0' + (k / 100) as u8);
        out.push(b'0' + (k % 100 / 10) as u8);
        out.push(b'0' + (k % 10) as u8);
    }
}

/// `format_buffer(digits, decimal_exponent, -4, 15)`.
fn format_buffer(digits: &[u8], dexp: i32, out: &mut Vec<u8>) {
    const MIN_EXP: i32 = -4;
    const MAX_EXP: i32 = 15;
    let k = digits.len() as i32;
    let n = k + dexp;
    if k <= n && n <= MAX_EXP {
        out.extend_from_slice(digits);
        out.resize(out.len() + (n - k) as usize, b'0');
        out.extend_from_slice(b".0");
    } else if 0 < n && n <= MAX_EXP {
        out.extend_from_slice(&digits[..n as usize]);
        out.push(b'.');
        out.extend_from_slice(&digits[n as usize..]);
    } else if MIN_EXP < n && n <= 0 {
        out.extend_from_slice(b"0.");
        out.resize(out.len() + (-n) as usize, b'0');
        out.extend_from_slice(digits);
    } else {
        out.push(digits[0]);
        if k > 1 {
            out.push(b'.');
            out.extend_from_slice(&digits[1..]);
        }
        out.push(b'e');
        append_exponent(out, n - 1);
    }
}

/// `to_chars(double)` as the writer prints a finite number (`-0.0` for negative zero).
pub fn to_chars(value: f64) -> String {
    let mut out = Vec::with_capacity(32);
    let mut v = value;
    if v.is_sign_negative() {
        v = -v;
        out.push(b'-');
    }
    if v == 0.0 {
        out.extend_from_slice(b"0.0");
        return String::from_utf8(out).unwrap_or_default();
    }
    let mut buf = [0u8; 32];
    let mut len = 0usize;
    let mut dexp = 0i32;
    grisu2(&mut buf, &mut len, &mut dexp, v);
    format_buffer(&buf[..len], dexp, &mut out);
    String::from_utf8(out).unwrap_or_default()
}
