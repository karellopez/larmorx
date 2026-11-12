// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! pandas' default float parser of `read_csv` (the C engine's `precise_xstrtod`, its
//! `float_precision=None`/`"high"` converter) and the column conversion around it
//! (`parsers.pyx`: `_try_double_nogil`), as pandas 3.0.6 and pandas 2.2.3 compute them
//! (`pandas/_libs/src/parser/tokenizer.c`; pandas is BSD-3-Clause, Copyright (c) 2008-2011
//! AQR Capital Management, LLC, Lambda Foundry, Inc. and PyData Development Team, and
//! 2011-2026 Open source contributors; `precise_xstrtod` is pandas' own code).
//!
//! fMRIPrep's confound steps read and write their tables with pandas 4 to 5 times in a row
//! (`FramewiseDisplacement`, `GatherConfounds`, `ExpandModel`, `SpikeRegressors`, `TSV2JSON`,
//! `FilterDropped`, `RenameACompCor`). `to_csv` writes each double as its shortest repr (up to
//! 17 significant digits); `precise_xstrtod` reads at most 17 *digit characters* (the leading
//! zeros of `0.000...` count), accumulates them in a double (`number * 10 + digit`, rounding
//! from the 16th digit on) and scales by one power of ten (`number / 1e{k}`), so most such
//! values come back one or more units in the last place off (PD-01). [`parse`] gives both
//! pandas' value and the correctly rounded value of the decimal the text denotes (larmorx's
//! default reading), for the same set of accepted texts.
//!
//! The two releases differ only in the exponent: pandas 3.0.6 reads it with C's `strtol`
//! (leading white space and any number of digits, a `long`: 64 bits on Linux x86-64), turns an
//! exponent above 308 into ±inf (or 0) and below -616 into +0; pandas 2.2.3 reads at most 17
//! digits right after the `e` and its sign into a 32-bit `int` (wrapping, as x86-64's
//! arithmetic does), and refuses an exponent above 308 or a result of ±inf (the token is then
//! not a float).
//!
//! Every operation is plain double arithmetic (no fused multiply-add: pandas' x86-64 wheels
//! are built for the baseline), so the results are the same on every platform.

/// The pandas release whose parser is followed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PandasRelease {
    /// pandas 2.2.x (2.2.3: fMRIPrep 25.2.6's image).
    V2_2,
    /// pandas 3.0.x (3.0.6: the newest, the default).
    V3_0,
}

impl PandasRelease {
    /// `"2.2"`, `"2.2.3"` (any 2.2.x), `"3.0"`, `"3.0.6"` (any 3.0.x); `None` otherwise.
    pub fn parse(version: &str) -> Option<PandasRelease> {
        let mut it = version.split('.');
        match (it.next(), it.next()) {
            (Some("2"), Some("2")) => Some(PandasRelease::V2_2),
            (Some("3"), Some("0")) => Some(PandasRelease::V3_0),
            _ => None,
        }
    }
}

/// pandas' default missing-value strings (`parsers.pyx`'s `STR_NA_VALUES`, the same in both
/// releases): a token equal to one of them is NaN.
pub const NA_VALUES: [&str; 19] = [
    "-1.#IND", "1.#QNAN", "1.#IND", "-1.#QNAN", "#N/A N/A", "#N/A", "N/A", "n/a", "NA", "<NA>",
    "#NA", "NULL", "null", "NaN", "-NaN", "nan", "-nan", "", "None",
];

/// Whether `word` is one of [`NA_VALUES`] (an exact, case-sensitive match).
pub fn is_na(word: &[u8]) -> bool {
    NA_VALUES.iter().any(|na| na.as_bytes() == word)
}

/// pandas' `isspace_ascii`: space, `\t`, `\n`, `\v`, `\f`, `\r`.
fn is_space(c: u8) -> bool {
    c == b' ' || (b'\t'..=b'\r').contains(&c)
}

/// The byte at `i`, or the terminating NUL past the end (the C string's view).
fn at(s: &[u8], i: usize) -> u8 {
    s.get(i).copied().unwrap_or(0)
}

/// The powers of ten `1e0 .. 1e308` of `precise_xstrtod`'s table (the compiler's correctly
/// rounded conversions of the literals; Rust's parser is correctly rounded as well).
fn pow10(k: usize) -> f64 {
    use std::sync::OnceLock;
    static TABLE: OnceLock<Vec<f64>> = OnceLock::new();
    TABLE.get_or_init(|| {
        (0..=308)
            .map(|k| format!("1e{k}").parse::<f64>().expect("a power of ten"))
            .collect()
    })[k]
}

/// What `precise_xstrtod` returns for one token.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Xstrtod {
    /// The double (meaningless when `error`).
    pub value: f64,
    /// `*endptr`'s offset (where parsing stopped, past trailing white space); `None` when the
    /// function returned before setting it (no digits, or pandas 2.2's exponent above 308).
    pub end: Option<usize>,
    /// `*error != 0` (`ERANGE`).
    pub error: bool,
}

/// The decimal the accepted text denotes, kept for the correctly rounded reading.
struct Denoted {
    negative: bool,
    digits: Vec<u8>,
    /// The power of ten of the last digit of `digits` (saturating).
    exp10: i64,
}

/// C's `strtol(p, &end, 10)` on a 64-bit `long` (glibc, the C locale): leading white space,
/// an optional sign, decimal digits; `(value, end offset, overflowed)`, the end offset being
/// `start` when there is no digit.
fn strtol(s: &[u8], start: usize) -> (i64, usize, bool) {
    let mut p = start;
    while matches!(at(s, p), b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r') {
        p += 1;
    }
    let mut neg = false;
    match at(s, p) {
        b'-' => {
            neg = true;
            p += 1;
        }
        b'+' => p += 1,
        _ => {}
    }
    let first = p;
    let mut acc: i128 = 0;
    let mut over = false;
    while at(s, p).is_ascii_digit() {
        if !over {
            acc = acc * 10 + i128::from(at(s, p) - b'0');
            if acc > i128::from(i64::MAX) + 1 {
                over = true;
            }
        }
        p += 1;
    }
    if p == first {
        return (0, start, false);
    }
    let v = if neg { -acc } else { acc };
    if over || v > i128::from(i64::MAX) {
        (if neg { i64::MIN } else { i64::MAX }, p, true)
    } else if v < i128::from(i64::MIN) {
        (i64::MIN, p, true)
    } else {
        (v as i64, p, false)
    }
}

/// pandas' `precise_xstrtod(word, &end, '.', 'E', '\0', 1, &error, NULL)` (decimal point `.`,
/// no thousands separator, trailing white space skipped), with the decimal the accepted text
/// denotes.
fn xstrtod(s: &[u8], release: PandasRelease) -> (Xstrtod, Denoted) {
    const MAX_DIGITS: i64 = 17;
    let mut p = 0usize;
    while is_space(at(s, p)) {
        p += 1;
    }
    let mut negative = false;
    match at(s, p) {
        b'-' => {
            negative = true;
            p += 1;
        }
        b'+' => p += 1,
        _ => {}
    }
    let mut den = Denoted {
        negative,
        digits: Vec::new(),
        exp10: 0,
    };
    let mut frac_digits: i64 = 0;
    let mut number = 0.0f64;
    // pandas 3.0: `long int` (64 bits); pandas 2.2: `int` (32 bits, wrapping).
    let mut exponent: i64 = 0;
    let mut num_digits: i64 = 0;
    let mut num_decimals: i64 = 0;
    let wrap = |v: i64| -> i64 {
        match release {
            PandasRelease::V2_2 => i64::from(v as i32),
            PandasRelease::V3_0 => v,
        }
    };
    while at(s, p).is_ascii_digit() {
        let d = at(s, p) - b'0';
        den.digits.push(d);
        if num_digits < MAX_DIGITS {
            number = number * 10.0 + f64::from(d);
            num_digits += 1;
        } else {
            exponent = wrap(exponent + 1);
        }
        p += 1;
    }
    if at(s, p) == b'.' {
        p += 1;
        while num_digits < MAX_DIGITS && at(s, p).is_ascii_digit() {
            let d = at(s, p) - b'0';
            den.digits.push(d);
            frac_digits += 1;
            number = number * 10.0 + f64::from(d);
            p += 1;
            num_digits += 1;
            num_decimals += 1;
        }
        if num_digits >= MAX_DIGITS {
            while at(s, p).is_ascii_digit() {
                den.digits.push(at(s, p) - b'0');
                frac_digits += 1;
                p += 1;
            }
        }
        exponent = wrap(exponent - num_decimals);
    }
    if num_digits == 0 {
        let out = Xstrtod {
            value: 0.0,
            end: None,
            error: true,
        };
        return (out, den);
    }
    if negative {
        number = -number;
    }
    let mut written: i64 = 0;
    if at(s, p).to_ascii_uppercase() == b'E' {
        p += 1;
        match release {
            PandasRelease::V3_0 => {
                let (n, end, overflow) = strtol(s, p);
                // `if (errno == ERANGE || checked_add(exponent, n, &exponent))`: on an overflow
                // the exponent is `n` alone.
                exponent = match (overflow, exponent.checked_add(n)) {
                    (false, Some(e)) => e,
                    _ => n,
                };
                written = n;
                if end == p {
                    p -= 1;
                } else {
                    p = end;
                }
            }
            PandasRelease::V2_2 => {
                let mut neg = false;
                match at(s, p) {
                    b'-' => {
                        neg = true;
                        p += 1;
                    }
                    b'+' => p += 1,
                    _ => {}
                }
                let mut nd: i64 = 0;
                let mut n: i32 = 0;
                while nd < MAX_DIGITS && at(s, p).is_ascii_digit() {
                    n = n.wrapping_mul(10).wrapping_add(i32::from(at(s, p) - b'0'));
                    nd += 1;
                    p += 1;
                }
                exponent = if neg {
                    i64::from((exponent as i32).wrapping_sub(n))
                } else {
                    i64::from((exponent as i32).wrapping_add(n))
                };
                written = if neg { -i64::from(n) } else { i64::from(n) };
                if nd == 0 {
                    p -= 1;
                }
            }
        }
    }
    den.exp10 = written.saturating_sub(frac_digits);
    if exponent > 308 {
        match release {
            PandasRelease::V3_0 => {
                number = if number == 0.0 {
                    0.0
                } else if number < 0.0 {
                    f64::NEG_INFINITY
                } else {
                    f64::INFINITY
                };
            }
            PandasRelease::V2_2 => {
                let out = Xstrtod {
                    value: f64::INFINITY,
                    end: None,
                    error: true,
                };
                return (out, den);
            }
        }
    } else if exponent > 0 {
        number *= pow10(exponent as usize);
    } else if exponent < -308 {
        if exponent < -616 {
            number = 0.0;
        } else {
            number /= pow10((-308 - exponent) as usize);
            number /= pow10(308);
        }
    } else {
        number /= pow10((-exponent) as usize);
    }
    let mut error = false;
    if release == PandasRelease::V2_2 && number.is_infinite() {
        error = true;
    }
    while is_space(at(s, p)) {
        p += 1;
    }
    (
        Xstrtod {
            value: number,
            end: Some(p),
            error,
        },
        den,
    )
}

/// pandas' `precise_xstrtod` of one token (see the module's documentation).
pub fn precise_xstrtod(word: &[u8], release: PandasRelease) -> Xstrtod {
    xstrtod(word, release).0
}

/// The correctly rounded double of a denoted decimal (Rust's parser is correctly rounded).
fn correctly_rounded(den: &Denoted) -> f64 {
    let first = den.digits.iter().position(|&d| d != 0);
    let v = match first {
        None => 0.0,
        Some(i) => {
            let digits: String = den.digits[i..]
                .iter()
                .map(|&d| char::from(b'0' + d))
                .collect();
            // Far beyond the double range either way: keep the text short.
            let e = den.exp10.clamp(-1_000_000_000, 1_000_000_000);
            format!("{digits}e{e}").parse::<f64>().expect("a decimal")
        }
    };
    if den.negative { -v } else { v }
}

/// One token as pandas' `_try_double_nogil` takes it (NA filtering on, the default set):
/// `Some((pandas' value, the correctly rounded value))` when the token is a float (both NaN
/// for a missing value, both ±inf for pandas' infinity words), `None` when it is not (the
/// column is then not a float column).
pub fn parse(word: &[u8], release: PandasRelease) -> Option<(f64, f64)> {
    if is_na(word) {
        return Some((f64::NAN, f64::NAN));
    }
    let (x, den) = xstrtod(word, release);
    let consumed = matches!(x.end, Some(e) if e > 0 && e >= word.len());
    if !x.error && consumed {
        return Some((x.value, correctly_rounded(&den)));
    }
    let lower = word.to_ascii_lowercase();
    match lower.as_slice() {
        b"inf" | b"+inf" | b"infinity" | b"+infinity" => Some((f64::INFINITY, f64::INFINITY)),
        b"-inf" | b"-infinity" => Some((f64::NEG_INFINITY, f64::NEG_INFINITY)),
        _ => None,
    }
}

/// A float column (`_try_double`): pandas' values and the correctly rounded values, `None`
/// when a token is not a float.
pub fn try_double<'a, I>(words: I, release: PandasRelease) -> Option<(Vec<f64>, Vec<f64>)>
where
    I: IntoIterator<Item = &'a [u8]>,
{
    let it = words.into_iter();
    let (lo, _) = it.size_hint();
    let mut pd = Vec::with_capacity(lo);
    let mut ex = Vec::with_capacity(lo);
    for w in it {
        let (a, b) = parse(w, release)?;
        pd.push(a);
        ex.push(b);
    }
    Some((pd, ex))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pd3(s: &str) -> Option<f64> {
        parse(s.as_bytes(), PandasRelease::V3_0).map(|v| v.0)
    }
    fn pd2(s: &str) -> Option<f64> {
        parse(s.as_bytes(), PandasRelease::V2_2).map(|v| v.0)
    }
    fn exact(s: &str) -> Option<f64> {
        parse(s.as_bytes(), PandasRelease::V3_0).map(|v| v.1)
    }

    #[test]
    fn short_texts_are_exact_and_long_ones_are_truncated() {
        for s in [
            "0.5", "1.25", "-3", "100", "0.000123", "1e-05", "2.5E+3", " 7 ", "+4.",
        ] {
            assert_eq!(pd3(s), exact(s), "{s}");
            assert_eq!(pd3(s), Some(s.trim().parse::<f64>().unwrap()), "{s}");
        }
        // 17 digit characters, the leading zeros counted ("0" and "0012345678901234"): the
        // last three digits are dropped and the 14 kept are divided by 1e16.
        let s = "0.0012345678901234567";
        assert_eq!(pd3(s), Some(12345678901234.0 / 1e16));
        assert_eq!(exact(s), Some(0.0012345678901234567));
        assert_ne!(pd3(s), exact(s));
    }

    #[test]
    fn the_two_releases_differ_in_the_exponent() {
        assert_eq!(pd3("1e400"), Some(f64::INFINITY));
        assert_eq!(pd3("-1e400"), Some(f64::NEG_INFINITY));
        assert_eq!(pd3("-0e400").map(f64::to_bits), Some(0.0f64.to_bits()));
        assert_eq!(pd2("1e400"), None);
        assert_eq!(pd2("2e308"), None);
        assert_eq!(pd3("2e308"), Some(f64::INFINITY));
        assert_eq!(pd3("1e 5"), Some(1e5));
        assert_eq!(pd2("1e 5"), None);
        assert_eq!(pd3("1e-700").map(f64::to_bits), Some(0.0f64.to_bits()));
        assert_eq!(
            exact("-1e-700").map(f64::to_bits),
            Some((-0.0f64).to_bits())
        );
        assert_eq!(pd3("1e"), None);
        assert_eq!(pd3("1e+"), None);
        assert_eq!(pd2("1e-"), None);
    }

    #[test]
    fn words_and_missing_values() {
        assert!(pd3("n/a").unwrap().is_nan());
        assert!(pd3("").unwrap().is_nan());
        assert_eq!(pd3("INF"), Some(f64::INFINITY));
        assert_eq!(pd3("-Infinity"), Some(f64::NEG_INFINITY));
        assert_eq!(pd3("NAN"), None);
        assert_eq!(pd3(" n/a"), None);
        assert_eq!(pd3("."), None);
        assert_eq!(pd3("1.5x"), None);
        assert_eq!(pd3("1,5"), None);
        assert_eq!(PandasRelease::parse("2.2.3"), Some(PandasRelease::V2_2));
        assert_eq!(PandasRelease::parse("3.0.6"), Some(PandasRelease::V3_0));
        assert_eq!(PandasRelease::parse("2.1.0"), None);
    }
}
