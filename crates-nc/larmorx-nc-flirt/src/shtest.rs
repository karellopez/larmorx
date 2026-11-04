// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret.
//! bash's `[ ... ]` (the `test` builtin, bash 5.1's `test.c` as documented and as it behaves:
//! POSIX's rules for up to four arguments, the `-a`/`-o`/`!`/`(` grammar beyond, and its
//! messages), for the tests FSL's scripts make on their unquoted arguments: when a value with
//! blanks is split into words, `[` sees more words than the script wrote (FSL-211). The value is
//! the test's truth, or the message bash prints (`[: too many arguments`, ``[: syntax error: `-x'
//! unexpected``, ...), after which the test is false (status 2).

use std::path::Path;

const BINARY: [&str; 14] = [
    "=", "==", "!=", "<", ">", "-nt", "-ot", "-ef", "-eq", "-ne", "-lt", "-le", "-gt", "-ge",
];
const UNARY: [&str; 26] = [
    "-a", "-b", "-c", "-d", "-e", "-f", "-g", "-h", "-k", "-n", "-o", "-p", "-r", "-s", "-t", "-u",
    "-v", "-w", "-x", "-z", "-G", "-L", "-N", "-O", "-R", "-S",
];

fn binop(s: &str) -> bool {
    BINARY.contains(&s)
}

fn unop(s: &str) -> bool {
    UNARY.contains(&s)
}

/// bash's integer argument (`legal_number`: blanks around, a sign, decimal digits).
fn integer(s: &str) -> Result<i64, String> {
    let t = s.trim_matches([' ', '\t', '\n']);
    let d = t.strip_prefix(['+', '-']).unwrap_or(t);
    if d.is_empty() || !d.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("{s}: integer expression expected"));
    }
    t.parse::<i64>()
        .map_err(|_| format!("{s}: integer expression expected"))
}

fn mtime(p: &str) -> Option<std::time::SystemTime> {
    std::fs::metadata(p).ok()?.modified().ok()
}

fn binary(a: &str, op: &str, b: &str) -> Result<bool, String> {
    Ok(match op {
        "=" | "==" => a == b,
        "!=" => a != b,
        "<" => a.as_bytes() < b.as_bytes(),
        ">" => a.as_bytes() > b.as_bytes(),
        "-nt" => match (mtime(a), mtime(b)) {
            (Some(x), Some(y)) => x > y,
            (Some(_), None) => true,
            _ => false,
        },
        "-ot" => match (mtime(a), mtime(b)) {
            (Some(x), Some(y)) => x < y,
            (None, Some(_)) => true,
            _ => false,
        },
        "-ef" => match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        },
        _ => {
            let (x, y) = (integer(a)?, integer(b)?);
            match op {
                "-eq" => x == y,
                "-ne" => x != y,
                "-lt" => x < y,
                "-le" => x <= y,
                "-gt" => x > y,
                _ => x >= y,
            }
        }
    })
}

fn unary(op: &str, a: &str) -> bool {
    let p = Path::new(a);
    let meta = std::fs::metadata(p);
    match op {
        "-n" => !a.is_empty(),
        "-z" => a.is_empty(),
        "-a" | "-e" => meta.is_ok(),
        "-f" => meta.is_ok_and(|m| m.is_file()),
        "-d" => meta.is_ok_and(|m| m.is_dir()),
        "-s" => meta.is_ok_and(|m| m.len() > 0),
        "-h" | "-L" => std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink()),
        "-r" | "-w" | "-x" => meta.is_ok(),
        _ => false,
    }
}

struct Parser<'a> {
    w: &'a [String],
    pos: usize,
}

impl Parser<'_> {
    fn or(&mut self) -> Result<bool, String> {
        let mut v = self.and()?;
        while self.pos < self.w.len() && self.w[self.pos] == "-o" {
            self.pos += 1;
            let r = self.and()?;
            v = v || r;
        }
        Ok(v)
    }

    fn and(&mut self) -> Result<bool, String> {
        let mut v = self.term()?;
        while self.pos < self.w.len() && self.w[self.pos] == "-a" {
            self.pos += 1;
            let r = self.term()?;
            v = v && r;
        }
        Ok(v)
    }

    fn term(&mut self) -> Result<bool, String> {
        let n = self.w.len();
        if self.pos >= n {
            return Err("argument expected".to_owned());
        }
        let a = self.w[self.pos].as_str();
        if a == "!" {
            self.pos += 1;
            return self.term().map(|v| !v);
        }
        if a == "(" {
            self.pos += 1;
            let v = self.or()?;
            if self.pos >= n || self.w[self.pos] != ")" {
                return Err("`)' expected".to_owned());
            }
            self.pos += 1;
            return Ok(v);
        }
        if self.pos + 3 <= n && binop(&self.w[self.pos + 1]) {
            let v = binary(a, &self.w[self.pos + 1], &self.w[self.pos + 2])?;
            self.pos += 3;
            return Ok(v);
        }
        if unop(a) {
            if self.pos + 1 >= n {
                return Err(format!("{a}: unary operator expected"));
            }
            let v = unary(a, &self.w[self.pos + 1]);
            self.pos += 2;
            return Ok(v);
        }
        self.pos += 1;
        Ok(!a.is_empty())
    }
}

fn two(w: &[String]) -> Result<bool, String> {
    if w[0] == "!" {
        return Ok(w[1].is_empty());
    }
    if unop(&w[0]) {
        return Ok(unary(&w[0], &w[1]));
    }
    Err(format!("{}: unary operator expected", w[0]))
}

fn three(w: &[String]) -> Result<bool, String> {
    if binop(&w[1]) {
        return binary(&w[0], &w[1], &w[2]);
    }
    if w[0] == "!" {
        return two(&w[1..]).map(|v| !v);
    }
    if w[0] == "(" && w[2] == ")" {
        return Ok(!w[1].is_empty());
    }
    match w[1].as_str() {
        "-a" => Ok(!w[0].is_empty() && !w[2].is_empty()),
        "-o" => Ok(!w[0].is_empty() || !w[2].is_empty()),
        _ => Err(format!("{}: binary operator expected", w[1])),
    }
}

/// `[ w... ]`: the test's truth, or the message bash prints after `[: ` (the test is then
/// false).
pub fn test(w: &[String]) -> Result<bool, String> {
    let n = w.len();
    match n {
        0 => return Ok(false),
        1 => return Ok(!w[0].is_empty()),
        2 => return two(w),
        3 => return three(w),
        4 if w[0] == "!" => return three(&w[1..]).map(|v| !v),
        4 if w[0] == "(" && w[3] == ")" => return two(&w[1..3]),
        _ => {}
    }
    let mut p = Parser { w, pos: 0 };
    let v = p.or()?;
    if p.pos != n {
        let a = &w[p.pos];
        return Err(if a.starts_with('-') {
            format!("syntax error: `{a}' unexpected")
        } else {
            "too many arguments".to_owned()
        });
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> Result<bool, String> {
        let w: Vec<String> = s.split(' ').map(str::to_owned).collect();
        test(&w)
    }

    /// bash 5.1.16's messages (`[ _$x != _ ]` with the values split, run 2026-11-04).
    #[test]
    fn bash_messages() {
        assert_eq!(t("_a b != _"), Err("too many arguments".into()));
        assert_eq!(t("_a -b != _"), Err("syntax error: `-b' unexpected".into()));
        assert_eq!(
            t("_-d -maskNONE != _"),
            Err("syntax error: `-maskNONE' unexpected".into())
        );
        assert_eq!(t("_a -a != _"), Err("too many arguments".into()));
        assert_eq!(t("_a -o != _"), Err("too many arguments".into()));
        assert_eq!(t("_a b c != _"), Err("too many arguments".into()));
        assert_eq!(
            t("_a -b c != _"),
            Err("syntax error: `-b' unexpected".into())
        );
        assert_eq!(t("_a = b != _"), Err("too many arguments".into()));
        assert_eq!(
            t("_a -eq b != _"),
            Err("_a: integer expression expected".into())
        );
        assert_eq!(t("_a ! != _"), Err("too many arguments".into()));
        assert_eq!(t("_a -nt != _"), Err("too many arguments".into()));
        assert_eq!(t("_a -z != _"), Err("syntax error: `-z' unexpected".into()));
        assert_eq!(
            t("_-x -y -z != _"),
            Err("syntax error: `-y' unexpected".into())
        );
        assert_eq!(t("_a != _"), Ok(true));
        assert_eq!(t("_ = _"), Ok(true));
        assert_eq!(t("-d = -d"), Ok(true));
        assert_eq!(t("! = -d"), Ok(false));
        assert_eq!(t("_a -a b != _"), Ok(true));
    }
}
