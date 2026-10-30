// SPDX-License-Identifier: Apache-2.0
//! The option parser of FSL's programs as `bet2` and `betsurf` show it (`specs/bet.md` §4.1,
//! black-box runs): `--name=value` long options, runs of one-letter options taking their
//! values from the following words, numbers read as C's `strtod` reads them.

/// One option.
#[derive(Clone, Copy, Debug)]
pub struct Spec {
    /// The one-letter form (`'m'` for `-m`), if any.
    pub short: Option<char>,
    /// The long form without its dashes, if any.
    pub long: Option<&'static str>,
    /// How many values it takes (0: a flag).
    pub values: usize,
    /// The values are numbers (else text).
    pub numeric: bool,
}

/// What an option was given.
#[derive(Clone, Debug, PartialEq)]
pub enum Given {
    Flag,
    Numbers(Vec<f64>),
    Text(String),
}

/// A parse error, with FSL's message (printed after the usage and an empty line).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError(pub String);

/// C's `strtod` on the whole word (leading blanks allowed; decimal, hexadecimal, `inf`,
/// `nan`), rounded to float as FSL stores its options: `None` when the word is not all number.
pub fn strtod_whole(word: &str) -> Option<f64> {
    let (v, used) = strtod(word);
    (used > 0 && used == word.len()).then(|| f64::from(v as f32))
}

/// C's `atof`: the longest leading number, or 0 (rounded to float).
pub fn atof(word: &str) -> f64 {
    f64::from(strtod(word).0 as f32)
}

/// C's `strtod`: the value and how many bytes it read (0 when none).
pub fn strtod(word: &str) -> (f64, usize) {
    let b = word.as_bytes();
    let mut i = 0;
    while i < b.len() && (b[i] == b' ' || (b'\t'..=b'\r').contains(&b[i])) {
        i += 1;
    }
    let start = i;
    let mut neg = false;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        neg = b[i] == b'-';
        i += 1;
    }
    let sign = |v: f64| if neg { -v } else { v };
    let rest = &word[i..];
    let lower = rest.to_ascii_lowercase();
    if lower.starts_with("infinity") {
        return (sign(f64::INFINITY), i + 8);
    }
    if lower.starts_with("inf") {
        return (sign(f64::INFINITY), i + 3);
    }
    if lower.starts_with("nan") {
        let mut j = i + 3;
        if j < b.len() && b[j] == b'(' {
            let mut k = j + 1;
            while k < b.len() && (b[k].is_ascii_alphanumeric() || b[k] == b'_') {
                k += 1;
            }
            if k < b.len() && b[k] == b')' {
                j = k + 1;
            }
        }
        return (sign(f64::NAN), j);
    }
    if lower.starts_with("0x") {
        // hexadecimal: digits, an optional point, an optional binary exponent
        let mut j = i + 2;
        let mut mant = 0f64;
        let mut digits = 0;
        let mut exp: i64 = 0;
        while j < b.len() && b[j].is_ascii_hexdigit() {
            mant = mant * 16.0 + f64::from((b[j] as char).to_digit(16).unwrap_or(0));
            digits += 1;
            j += 1;
        }
        if j < b.len() && b[j] == b'.' {
            j += 1;
            while j < b.len() && b[j].is_ascii_hexdigit() {
                mant = mant * 16.0 + f64::from((b[j] as char).to_digit(16).unwrap_or(0));
                exp -= 4;
                digits += 1;
                j += 1;
            }
        }
        if digits == 0 {
            // "0x" alone reads as 0 followed by "x"
            return (sign(0.0), i + 1);
        }
        if j < b.len() && (b[j] == b'p' || b[j] == b'P') {
            let mut k = j + 1;
            let mut eneg = false;
            if k < b.len() && (b[k] == b'+' || b[k] == b'-') {
                eneg = b[k] == b'-';
                k += 1;
            }
            let es = k;
            let mut e: i64 = 0;
            while k < b.len() && b[k].is_ascii_digit() {
                e = (e * 10 + i64::from(b[k] - b'0')).min(100_000);
                k += 1;
            }
            if k > es {
                exp += if eneg { -e } else { e };
                j = k;
            }
        }
        return (sign(mant * 2f64.powi(exp.clamp(-2000, 2000) as i32)), j);
    }
    let mut j = i;
    let digits_start = j;
    while j < b.len() && b[j].is_ascii_digit() {
        j += 1;
    }
    let mut digits = j - digits_start;
    if j < b.len() && b[j] == b'.' {
        let k0 = j + 1;
        let mut k = k0;
        while k < b.len() && b[k].is_ascii_digit() {
            k += 1;
        }
        digits += k - k0;
        if digits > 0 {
            j = k;
        }
    }
    if digits == 0 {
        return (0.0, 0);
    }
    if j < b.len() && (b[j] == b'e' || b[j] == b'E') {
        let mut k = j + 1;
        if k < b.len() && (b[k] == b'+' || b[k] == b'-') {
            k += 1;
        }
        let es = k;
        while k < b.len() && b[k].is_ascii_digit() {
            k += 1;
        }
        if k > es {
            j = k;
        }
    }
    let _ = start;
    let v: f64 = word[i..j].parse().unwrap_or(0.0);
    (sign(v), j)
}

/// Parses `words` (after the program's leading names) against `specs`. `stop_at_word`: the
/// first word not starting with `-` ends the options (betsurf) instead of being an error.
/// Returns what was given for each spec (the last of repeated options) and the words after
/// the options.
pub fn parse(
    words: &[String],
    specs: &[Spec],
    stop_at_word: bool,
) -> Result<(Vec<Option<Given>>, usize), ParseError> {
    let mut given: Vec<Option<Given>> = vec![None; specs.len()];
    let mut i = 0;
    while i < words.len() {
        let w = &words[i];
        if let Some(long) = w.strip_prefix("--") {
            let (name, value) = match long.split_once('=') {
                Some((n, v)) => (n, Some(v)),
                None => (long, None),
            };
            let shown = format!("--{name}");
            let k = specs
                .iter()
                .position(|s| s.long == Some(name))
                .ok_or_else(|| ParseError(format!("{shown}: Option doesn't exist!")))?;
            let s = specs[k];
            i += 1;
            if s.values == 0 {
                given[k] = Some(Given::Flag);
                continue;
            }
            let value = match value {
                Some(v) if !v.is_empty() => v,
                _ => return Err(ParseError(format!("{shown}: Missing non-optional argument!"))),
            };
            given[k] = Some(if s.numeric {
                let vals: Vec<&str> = value.split_whitespace().collect();
                let all: Option<Vec<f64>> = if vals.len() == s.values {
                    vals.iter().map(|v| strtod_whole(v)).collect()
                } else {
                    None
                };
                Given::Numbers(all.ok_or_else(|| {
                    ParseError(format!("{shown}: Couldn't set_value! valstr=\"{value}\"!"))
                })?)
            } else {
                Given::Text(value.to_owned())
            });
        } else if let Some(run) = w.strip_prefix('-').filter(|r| !r.is_empty()) {
            i += 1;
            for c in run.chars() {
                let shown = format!("-{c}");
                let k = specs
                    .iter()
                    .position(|s| s.short == Some(c))
                    .ok_or_else(|| ParseError(format!("{shown}: Option doesn't exist!")))?;
                let s = specs[k];
                if s.values == 0 {
                    given[k] = Some(Given::Flag);
                    continue;
                }
                let rest = &words[i..];
                if rest.is_empty() {
                    return Err(ParseError(format!("{shown}: Missing non-optional argument!")));
                }
                if !s.numeric {
                    given[k] = Some(Given::Text(rest[0].clone()));
                    i += 1;
                    continue;
                }
                if rest.len() < s.values {
                    // fewer words than values: each read as atof, the others 0 (observed)
                    let mut vals: Vec<f64> = rest.iter().map(|v| atof(v)).collect();
                    vals.resize(s.values, 0.0);
                    given[k] = Some(Given::Numbers(vals));
                    i = words.len();
                    continue;
                }
                // the last value must be a whole number, the others read as atof (observed)
                let take = &rest[..s.values];
                let last = strtod_whole(&take[s.values - 1]);
                match last {
                    Some(l) => {
                        let mut vals: Vec<f64> =
                            take[..s.values - 1].iter().map(|v| atof(v)).collect();
                        vals.push(l);
                        given[k] = Some(Given::Numbers(vals));
                        i += s.values;
                    }
                    None => {
                        let shown_words = rest[..(s.values + 1).min(rest.len())].join(" ");
                        return Err(ParseError(format!(
                            "{shown}: Couldn't set_value! valstr=\"{shown_words}\"!"
                        )));
                    }
                }
            }
        } else if stop_at_word {
            return Ok((given, i));
        } else {
            return Err(ParseError(format!("{w}:  is an unrecognised token!")));
        }
    }
    Ok((given, i))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| (*x).to_owned()).collect()
    }

    const SPECS: [Spec; 3] = [
        Spec {
            short: Some('m'),
            long: Some("mask"),
            values: 0,
            numeric: false,
        },
        Spec {
            short: Some('f'),
            long: None,
            values: 1,
            numeric: true,
        },
        Spec {
            short: Some('c'),
            long: None,
            values: 3,
            numeric: true,
        },
    ];

    #[test]
    fn like_fsl() {
        let (g, _) = parse(&w(&["-fm", "0.3"]), &SPECS, false).unwrap();
        assert_eq!(g[1], Some(Given::Numbers(vec![f64::from(0.3f32)])));
        assert_eq!(g[0], Some(Given::Flag));
        let e = parse(&w(&["-f", "abc", "-m"]), &SPECS, false).unwrap_err();
        assert_eq!(e.0, "-f: Couldn't set_value! valstr=\"abc -m\"!");
        let e = parse(&w(&["-c", "1", "2", "-v3", "-m", "-t"]), &SPECS, false).unwrap_err();
        assert_eq!(e.0, "-c: Couldn't set_value! valstr=\"1 2 -v3 -m\"!");
        let (g, _) = parse(&w(&["-c", "1", "2x", "3"]), &SPECS, false).unwrap();
        assert_eq!(g[2], Some(Given::Numbers(vec![1.0, 2.0, 3.0])));
        let (g, _) = parse(&w(&["-c", "4"]), &SPECS, false).unwrap();
        assert_eq!(g[2], Some(Given::Numbers(vec![4.0, 0.0, 0.0])));
        let e = parse(&w(&["extra"]), &SPECS, false).unwrap_err();
        assert_eq!(e.0, "extra:  is an unrecognised token!");
        let e = parse(&w(&["-x"]), &SPECS, false).unwrap_err();
        assert_eq!(e.0, "-x: Option doesn't exist!");
        assert_eq!(strtod_whole("0x1p-1"), Some(0.5));
        assert_eq!(strtod_whole(" 0.5"), Some(0.5));
        assert_eq!(strtod_whole("0.5x"), None);
        assert!(strtod_whole("nan").unwrap().is_nan());
        assert_eq!(strtod_whole("-inf"), Some(f64::NEG_INFINITY));
        assert_eq!(strtod_whole("-.5"), Some(-0.5));
        assert_eq!(strtod_whole("-"), None);
        assert_eq!(strtod_whole("-e5"), None);
    }
}
