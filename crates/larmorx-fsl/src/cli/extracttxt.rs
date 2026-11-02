// SPDX-License-Identifier: Apache-2.0
//! `larmorx fsl extracttxt`: FSL's `extracttxt` script (`specs/flirt.md` §15.8), the lines of a
//! text file from the first that matches a word, as its shell script computes them with GNU
//! grep, dc and sed.

use std::io::Write;

use super::Settings;
use super::bre::Bre;
use super::common::refuse;

/// The usage line (observed: standard output).
pub const USAGE: &str =
    "Usage: extracttxt search_word file [num_trailing_lines [relative_start]]\n";

/// dash's message when the script's `exit -1` runs (standard error).
pub const ILLEGAL: &str = "extracttxt: 68: exit: Illegal number: -1\n";

/// What GNU sed prints for a range read as its options; its usage text (GNU sed's own, GPL)
/// is not reproduced (`docs/findings/fsl-flirt-original.md`).
const SED_USAGE: &str = "larmorx fsl extracttxt: GNU sed prints its usage here (not reproduced: GNU sed's own text) (larmorx)\n";

/// An integer word as the script reads it: an optional sign and digits.
fn integer(w: &str) -> Option<(bool, i128)> {
    let (sign, digits) = match w.as_bytes().first() {
        Some(b'+' | b'-') => (true, &w[1..]),
        _ => (false, w),
    };
    if digits.is_empty() || digits.len() > 18 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((sign, digits.parse().ok()?))
}

/// The value of an integer word as dash's `test` reads it.
fn value(w: &str) -> i128 {
    let (_, d) = integer(w).unwrap_or((false, 0));
    if w.starts_with('-') { -d } else { d }
}

/// A word as dc reads it after a number already on its stack is used up: a leading sign is a
/// command with too few operands (`dc: stack empty`, nothing done), the digits a number.
fn dc_operand(w: &str, err: &mut dyn Write) -> i128 {
    let (sign, d) = integer(w).unwrap_or((false, 0));
    if sign {
        let _ = writeln!(err, "dc: stack empty");
    }
    d
}

/// Whether a shell glob in `word` would match a name in the directory (the script passes its
/// arguments unquoted: the shell would replace the word by the names).
fn glob_matches(word: &str, settings: &Settings) -> bool {
    if !word.contains(['*', '?', '[']) {
        return false;
    }
    let (dir, pat) = match word.rfind('/') {
        Some(k) => (&word[..=k], &word[k + 1..]),
        None => ("", word),
    };
    if dir.contains(['*', '?', '[']) {
        return true;
    }
    let path = settings.path(if dir.is_empty() { "." } else { dir });
    let Ok(entries) = std::fs::read_dir(path) else {
        return false;
    };
    entries.flatten().any(|e| {
        let name = e.file_name().to_string_lossy().into_owned();
        !(name.starts_with('.') && !pat.starts_with('.')) && fnmatch(pat.as_bytes(), name.as_bytes())
    })
}

/// The shell's pattern matching (`*`, `?`, `[...]` with `!` or `^`).
fn fnmatch(p: &[u8], s: &[u8]) -> bool {
    match p.first() {
        None => s.is_empty(),
        Some(b'*') => (0..=s.len()).any(|k| fnmatch(&p[1..], &s[k..])),
        Some(b'?') => !s.is_empty() && fnmatch(&p[1..], &s[1..]),
        Some(b'[') => {
            let Some(close) = p.iter().skip(2).position(|&c| c == b']').map(|k| k + 2) else {
                return !s.is_empty() && s[0] == b'[' && fnmatch(&p[1..], &s[1..]);
            };
            let Some(&c) = s.first() else { return false };
            let mut body = &p[1..close];
            let negate = matches!(body.first(), Some(b'!' | b'^'));
            if negate {
                body = &body[1..];
            }
            let mut hit = false;
            let mut i = 0;
            while i < body.len() {
                if i + 2 < body.len() && body[i + 1] == b'-' {
                    hit |= body[i] <= c && c <= body[i + 2];
                    i += 3;
                } else {
                    hit |= body[i] == c;
                    i += 1;
                }
            }
            hit != negate && fnmatch(&p[close + 1..], &s[1..])
        }
        Some(&c) => !s.is_empty() && s[0] == c && fnmatch(&p[1..], &s[1..]),
    }
}

/// `extracttxt` with FSL's arguments; returns the exit status.
pub fn main(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    if args.len() < 2 {
        let _ = write!(out, "{USAGE}");
        let _ = write!(err, "{ILLEGAL}");
        return 2;
    }
    let (wordarg, file) = (&args[0], &args[1]);
    let count = args.get(2);
    let start = args.get(3);
    let blank = |w: &str| w.is_empty() || w.contains([' ', '\t', '\n']);
    if blank(wordarg) || wordarg.starts_with('-') || blank(file) {
        return refuse(
            err,
            "extracttxt",
            "the search word and the file must be single words, the word not starting with '-' (the script passes them unquoted: the shell would split them, grep read them as options)",
        );
    }
    for w in [count, start].into_iter().flatten() {
        if integer(w).is_none() {
            return refuse(
                err,
                "extracttxt",
                &format!(
                    "'{w}' is not an integer of at most 18 digits (the script's arithmetic would be undefined)"
                ),
            );
        }
    }
    if args[..args.len().min(4)]
        .iter()
        .any(|w| glob_matches(w, settings))
    {
        return refuse(
            err,
            "extracttxt",
            "an argument is a shell pattern that matches names in the directory (the shell would replace it by the names)",
        );
    }
    let re = match Bre::new(wordarg.as_bytes()) {
        Ok(r) => r,
        Err(e) => {
            return refuse(
                err,
                "extracttxt",
                &format!(
                    "'{wordarg}' is not a basic regular expression ({e}: grep fails and the script goes on without a line)"
                ),
            );
        }
    };
    let path = settings.path(file);
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            let what = if path.is_dir() {
                "Is a directory".to_owned()
            } else {
                match e.kind() {
                    std::io::ErrorKind::NotFound => "No such file or directory".to_owned(),
                    std::io::ErrorKind::PermissionDenied => "Permission denied".to_owned(),
                    _ => e.to_string(),
                }
            };
            let _ = writeln!(err, "grep: {file}: {what}");
            return 0;
        }
    };
    if bytes.contains(&0) {
        return refuse(
            err,
            "extracttxt",
            &format!("{file} holds a NUL byte (grep reads it as a binary file)"),
        );
    }
    let mut lines: Vec<&[u8]> = bytes.split_inclusive(|&b| b == b'\n').collect();
    if lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    let body = |l: &[u8]| -> usize { l.len() - usize::from(l.ends_with(b"\n")) };
    let mut num = None;
    for (k, l) in lines.iter().enumerate() {
        match re.is_match(&l[..body(l)]) {
            Some(true) => {
                num = Some(k as i128 + 1);
                break;
            }
            Some(false) => {}
            None => {
                return refuse(
                    err,
                    "extracttxt",
                    &format!("'{wordarg}' takes too long to match (exponential backtracking)"),
                );
            }
        }
    }
    let Some(num) = num else {
        return 0;
    };
    // num1 = num + start (a negative start subtracted; any other sign a dc command)
    let mut num1 = match start {
        Some(k) if value(k) < 0 => num + value(k),
        Some(k) => num + dc_operand(k, err),
        None => num,
    };
    let mut num2 = None;
    if let Some(c) = count {
        // dc reads num1 back: a negative one's sign is a command on an empty stack
        if num1 < 0 {
            let _ = writeln!(err, "dc: stack empty");
        }
        let n2 = num1.abs() + dc_operand(c, err) - 1;
        if n2 < num1 {
            num1 = n2;
        }
        num2 = Some(n2);
    }
    let expr = match num2 {
        Some(n2) => format!("{num1},{n2}p"),
        None => format!("{num1},$p"),
    };
    if num1 < 0 {
        let first = expr.as_bytes()[1] as char;
        let _ = write!(err, "sed: invalid option -- '{first}'\n{SED_USAGE}");
        return 1;
    }
    if num1 == 0 {
        let _ = writeln!(
            err,
            "sed: -e expression #1, char {}: invalid usage of line address 0",
            expr.len()
        );
        return 1;
    }
    let first = (num1 - 1) as usize;
    let last = num2.map_or(lines.len(), |n| (n.max(num1) as usize).min(lines.len()));
    for l in lines.iter().take(last).skip(first) {
        let _ = out.write_all(l);
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(words: &[&str]) -> (u8, String, String) {
        let dir = tempfile::tempdir().unwrap();
        let txt: String = (1..13)
            .map(|k| format!("line {k} value {}\n", k * 3))
            .chain(["the end\n".to_owned()])
            .collect();
        std::fs::write(dir.path().join("t.txt"), txt).unwrap();
        let args: Vec<String> = words.iter().map(|s| (*s).to_owned()).collect();
        let settings = Settings {
            dir: Some(dir.path().to_path_buf()),
            ..Settings::default()
        };
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = main(&args, &settings, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    #[test]
    fn as_the_script_prints() {
        // FSL 6.0.7.23's script (observed)
        assert_eq!(
            run(&["e.4", "t.txt", "3"]),
            (0, "line 4 value 12\nline 5 value 15\nline 6 value 18\n".into(), String::new())
        );
        assert_eq!(
            run(&["end$", "t.txt", "4", "-3"]).1,
            "line 10 value 30\nline 11 value 33\nline 12 value 36\nthe end\n"
        );
        assert_eq!(run(&["line.7", "t.txt", "0"]).1, "line 6 value 18\n");
        assert_eq!(
            run(&["e.4", "t.txt", "-0", "+2"]),
            (0, "line 5 value 15\n".into(), "dc: stack empty\ndc: stack empty\n".into())
        );
        let (c, _, err) = run(&["^line.1.v", "t.txt", "2", "-1"]);
        assert_eq!(
            (c, err.as_str()),
            (1, "sed: -e expression #1, char 4: invalid usage of line address 0\n")
        );
        let (c, _, err) = run(&["e.2", "t.txt", "3", "-4"]);
        assert_eq!(c, 1);
        assert!(err.starts_with("dc: stack empty\nsed: invalid option -- '2'\n"), "{err}");
        let (c, _, err) = run(&["e.4", "t.txt", "0", "-4"]);
        assert_eq!(c, 1);
        assert!(err.starts_with("sed: invalid option -- '1'\n"), "{err}");
        assert_eq!(run(&["nothing", "t.txt"]), (0, String::new(), String::new()));
        assert_eq!(
            run(&["x", "none.txt"]),
            (0, String::new(), "grep: none.txt: No such file or directory\n".into())
        );
        assert_eq!(run(&["word"]), (2, USAGE.into(), ILLEGAL.into()));
        assert_eq!(run(&["c", "t.txt", "x"]).0, 1);
    }
}
