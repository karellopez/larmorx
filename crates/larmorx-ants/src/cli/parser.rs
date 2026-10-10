// SPDX-License-Identifier: Apache-2.0
//! ANTs' command-line parser (`Utilities/antsCommandLineParser.cxx` and
//! `antsCommandLineOption.cxx`, ANTs v2.6.5), so arguments are read exactly as ANTs reads them.
//!
//! - Arguments in brackets may contain spaces: `[a, 1]` given as two words is regrouped.
//!   `{`, `(` and `<` open and `}`, `)` and `>` close brackets too.
//! - `--name` is a long option. `-x` is a short one: only one character counts, and `-xy`
//!   is read as the long option `xy`.
//! - The words after an option are its values, up to the next word that starts with `-`.
//!   Negative numbers (`-0.5`) are values, not options. An option with no value gets `1`.
//! - A value `name[p1,p2]` has a name and parameters.
//! - Options may repeat. Tools read the **last** occurrence of single-valued options.

/// One value of an option: `name` or `name[p1,p2,...]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptionValue {
    pub name: String,
    pub parameters: Vec<String>,
}

impl OptionValue {
    fn parse(text: &str) -> Self {
        match (text.find('['), text.find(']')) {
            (Some(left), Some(right)) => {
                // ANTs splits on every comma up to the first ']', without nesting.
                let mut parameters = Vec::new();
                let mut start = left + 1;
                while let Some(comma) = text[start..].find(',').map(|c| start + c) {
                    parameters.push(text[start..comma].to_owned());
                    start = comma + 1;
                }
                parameters.push(text.get(start..right).unwrap_or("").to_owned());
                OptionValue {
                    name: text[..left].to_owned(),
                    parameters,
                }
            }
            _ => OptionValue {
                name: text.to_owned(),
                parameters: Vec::new(),
            },
        }
    }
}

/// An option a tool accepts.
#[derive(Clone, Copy, Debug)]
pub struct OptionSpec {
    pub long: &'static str,
    pub short: Option<char>,
}

/// Parsed arguments: for each option (by long name), its values in command-line order.
#[derive(Clone, Debug, Default)]
pub struct Parsed {
    values: Vec<(&'static str, Vec<OptionValue>)>,
}

impl Parsed {
    /// All values of an option, in command-line order.
    pub fn all(&self, long: &str) -> &[OptionValue] {
        self.values
            .iter()
            .find(|(name, _)| *name == long)
            .map_or(&[], |(_, v)| v.as_slice())
    }

    /// The value ANTs reads for a single-valued option (`GetFunction(0)`: the last one given).
    pub fn last(&self, long: &str) -> Option<&OptionValue> {
        self.all(long).last()
    }

    pub fn has(&self, long: &str) -> bool {
        !self.all(long).is_empty()
    }
}

/// C's `atof` (glibc's `strtod`): the number at the start of `s` (0 when there is none),
/// including `inf`, `infinity`, `nan` (any case) and hexadecimal floats (`0x1.8p3`).
pub fn atof(s: &str) -> f64 {
    let s = s.trim_start();
    let bytes = s.as_bytes();
    let mut end = 0;
    if matches!(bytes.first(), Some(b'+' | b'-')) {
        end = 1;
    }
    let negative = bytes.first() == Some(&b'-');
    let sign = if negative { -1.0 } else { 1.0 };
    let rest = s[end..].to_ascii_lowercase();
    if rest.starts_with("inf") {
        return sign * f64::INFINITY;
    }
    if rest.starts_with("nan") {
        return if negative { -f64::NAN } else { f64::NAN };
    }
    if let Some(hex) = rest.strip_prefix("0x") {
        return sign * hex_float(hex);
    }
    let digits_from = end;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
    }
    let mut digits = end - digits_from;
    if bytes.get(end) == Some(&b'.') {
        end += 1;
        let from = end;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
        digits += end - from;
    }
    if digits == 0 {
        return 0.0;
    }
    if matches!(bytes.get(end), Some(b'e' | b'E')) {
        let mut e = end + 1;
        if matches!(bytes.get(e), Some(b'+' | b'-')) {
            e += 1;
        }
        let from = e;
        while bytes.get(e).is_some_and(u8::is_ascii_digit) {
            e += 1;
        }
        if e > from {
            end = e;
        }
    }
    s[..end].parse().unwrap_or(0.0)
}

/// The value of the hexadecimal float after `0x` (`strtod`): hex digits with an optional
/// point, then an optional binary exponent `p±d`. Without a hex digit only the `0` counts.
/// Exact for up to 28 significant hex digits.
fn hex_float(s: &str) -> f64 {
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut mantissa: u128 = 0;
    let mut exponent: i64 = 0;
    let mut digits = 0;
    let mut point = false;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'.' && !point {
            point = true;
        } else if let Some(d) = (c as char).to_digit(16) {
            digits += 1;
            if mantissa < (1u128 << 112) {
                mantissa = mantissa * 16 + u128::from(d);
                if point {
                    exponent -= 4;
                }
            } else if !point {
                exponent += 4;
            }
        } else {
            break;
        }
        i += 1;
    }
    if digits == 0 {
        return 0.0;
    }
    if matches!(bytes.get(i), Some(b'p' | b'P')) {
        let mut j = i + 1;
        let negative = bytes.get(j) == Some(&b'-');
        if matches!(bytes.get(j), Some(b'+' | b'-')) {
            j += 1;
        }
        let from = j;
        let mut e: i64 = 0;
        while let Some(d) = bytes.get(j).filter(|b| b.is_ascii_digit()) {
            e = (e * 10 + i64::from(d - b'0')).min(100_000);
            j += 1;
        }
        if j > from {
            exponent += if negative { -e } else { e };
        }
    }
    let exp = i32::try_from(exponent.clamp(-100_000, 100_000)).unwrap_or(0);
    // Scale in steps that stay within the f64 exponent range.
    let mut value = mantissa as f64;
    let mut e = exp;
    while e > 0 {
        let step = e.min(1000);
        value *= 2f64.powi(step);
        e -= step;
    }
    while e < 0 {
        let step = (-e).min(1000);
        value /= 2f64.powi(step);
        e += step;
    }
    value
}

/// ANTs' `RegroupCommandLineArguments`: joins bracketed values split across words.
fn regroup(args: &[String]) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut open = false;
    for arg in args {
        let mut a: Vec<char> = arg.chars().collect();
        if let Some(first) = a.first_mut()
            && matches!(*first, '{' | '(' | '<')
        {
            *first = '[';
        }
        if let Some(last) = a.last_mut()
            && matches!(*last, '}' | ')' | '>')
        {
            *last = ']';
        }
        let a: String = a.into_iter().collect();
        let (left, right) = (a.find('['), a.find(']'));
        let bad = || format!("Incorrect command line specification: {a}");
        if open {
            if left.is_some() {
                return Err(bad());
            }
            current.push_str(&a);
            if let Some(r) = right {
                if r + 1 < a.len() {
                    return Err(bad());
                }
                out.push(std::mem::take(&mut current));
                open = false;
            }
        } else {
            match (left, right) {
                (None, None) => out.push(a),
                (None, Some(_)) => return Err(bad()),
                (Some(l), Some(r)) if l < r => {
                    if r + 1 < a.len() {
                        return Err(bad());
                    }
                    out.push(a);
                }
                (Some(_), None) => {
                    current = a;
                    open = true;
                }
                // ']' before '[': ANTs drops the word.
                (Some(_), Some(_)) => {}
            }
        }
    }
    Ok(out)
}

/// Parses `args` (without the program name) for a tool accepting `specs`.
pub fn parse(args: &[String], specs: &[OptionSpec]) -> Result<Parsed, String> {
    let args = regroup(args)?;
    let mut parsed = Parsed {
        values: specs.iter().map(|s| (s.long, Vec::new())).collect(),
    };
    let mut invalid = Vec::new();
    let mut n = 0;
    while n < args.len() {
        let argument = &args[n];
        n += 1;
        let name: String = if let Some(long) = argument.strip_prefix("--") {
            long.to_owned()
        } else if let Some(short) = argument.strip_prefix('-') {
            short.chars().take(2).collect()
        } else {
            String::new()
        };
        if name.is_empty() || atof(&name) != 0.0 {
            continue;
        }
        let spec = specs.iter().find(|s| {
            let mut chars = name.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => s.short == Some(c),
                _ => s.long == name,
            }
        });
        // The option's values: the following words up to the next option.
        let mut values = Vec::new();
        while n < args.len() {
            let word = &args[n];
            if word.starts_with('-') && atof(word) == 0.0 {
                break;
            }
            values.push(OptionValue::parse(word));
            n += 1;
        }
        if values.is_empty() {
            values.push(OptionValue::parse("1"));
        }
        match spec {
            Some(spec) => {
                let slot = parsed
                    .values
                    .iter_mut()
                    .find(|(long, _)| *long == spec.long)
                    .expect("one slot per spec");
                slot.1.extend(values);
            }
            None => invalid.push(name),
        }
    }
    if invalid.is_empty() {
        Ok(parsed)
    } else {
        Err(format!("Invalid flag provided: {}", invalid.join(", ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPECS: &[OptionSpec] = &[
        OptionSpec {
            long: "transform",
            short: Some('t'),
        },
        OptionSpec {
            long: "default-value",
            short: Some('f'),
        },
        OptionSpec {
            long: "float",
            short: None,
        },
        OptionSpec {
            long: "interpolation",
            short: Some('n'),
        },
    ];

    fn args(s: &[&str]) -> Vec<String> {
        s.iter().map(|a| a.to_string()).collect()
    }

    #[test]
    fn values_brackets_and_repeats() {
        let p = parse(
            &args(&[
                "-t",
                "[a.mat,",
                "1]",
                "--transform",
                "w.nii.gz",
                "-n",
                "Gaussian[1x2x3,2]",
            ]),
            SPECS,
        )
        .unwrap();
        let t = p.all("transform");
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].name, "");
        assert_eq!(t[0].parameters, ["a.mat", "1"]);
        assert_eq!(t[1].name, "w.nii.gz");
        let n = p.last("interpolation").unwrap();
        assert_eq!(
            (n.name.as_str(), n.parameters.as_slice()),
            ("Gaussian", &["1x2x3".to_string(), "2".to_string()][..])
        );
    }

    #[test]
    fn only_first_and_last_characters_are_rewritten_as_brackets() {
        // As in ANTs, `(` inside a word is not a bracket, so `)` at the end has no partner.
        assert!(parse(&args(&["-n", "Gaussian(1x2x3,2)"]), SPECS).is_err());
        let p = parse(&args(&["-t", "(a.mat,1)"]), SPECS).unwrap();
        assert_eq!(p.last("transform").unwrap().parameters, ["a.mat", "1"]);
    }

    #[test]
    fn negative_numbers_are_values_and_bare_flags_are_one() {
        let p = parse(&args(&["-f", "-0.5", "--float"]), SPECS).unwrap();
        assert_eq!(p.last("default-value").unwrap().name, "-0.5");
        assert_eq!(p.last("float").unwrap().name, "1");
        assert!(parse(&args(&["--nope", "1"]), SPECS).is_err());
        // An unclosed bracket swallows the rest; the option is then a bare flag.
        let p = parse(&args(&["-t", "[a,1"]), SPECS).unwrap();
        assert_eq!(p.last("transform").unwrap().name, "1");
    }

    #[test]
    fn atof_reads_leading_numbers() {
        assert_eq!(atof("1x1x1"), 1.0);
        assert_eq!(atof("-2.5e1mm"), -25.0);
        assert_eq!(atof("t"), 0.0);
        assert_eq!(atof("1e"), 1.0);
        assert_eq!(atof(".5"), 0.5);
        // strtod also reads infinities, NaN and hexadecimal floats.
        assert_eq!(atof("inf"), f64::INFINITY);
        assert_eq!(atof("  -Infinity"), f64::NEG_INFINITY);
        assert!(atof("nan").is_nan());
        assert!(atof("-NaN(123)").is_sign_negative());
        assert_eq!(atof("0x10"), 16.0);
        assert_eq!(atof("0x1.8p1"), 3.0);
        assert_eq!(atof("-0x.4P-2"), -0.0625);
        assert_eq!(atof("0x"), 0.0);
        assert_eq!(atof("0xg"), 0.0);
    }
}
