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

/// C's `atof`: the number at the start of `s` (0 when there is none).
pub fn atof(s: &str) -> f64 {
    let s = s.trim_start();
    let bytes = s.as_bytes();
    let mut end = 0;
    if matches!(bytes.first(), Some(b'+' | b'-')) {
        end = 1;
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
    }
}
