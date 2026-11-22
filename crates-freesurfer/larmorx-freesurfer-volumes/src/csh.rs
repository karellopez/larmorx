// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! What tcsh does to an unquoted variable's words in FreeSurfer's tcsh scripts (`#!/bin/tcsh -f`),
//! for their replicas' `reproduce` mode: the behaviour of tcsh 6.21.00 as Ubuntu 22.04 builds it
//! (the machine the reference runs were made on), measured black-box on the official scripts and
//! on small test scripts (`validation/probes/fs-getfullpath/`); no tcsh code. With GNU coreutils'
//! `dirname`, `basename` and `pwd` (8.32) where the scripts run them.
//!
//! - **Splitting**: an unquoted `$v` gives each of `v`'s words split at blanks (space, tab), empty
//!   pieces dropped.
//! - **Expansion** of each piece, in order: braces (`a{b,c}d`: `abd acd`; `{}` and a lone `}` stay;
//!   an unclosed `{`: `Missing '}'.`), a leading `~` (`$HOME`, or `~user`'s home from the password
//!   file: `Unknown user: <user>.`), a leading `=N` or `=-` (the directory stack, which holds the
//!   current folder and, after a `pushd`, the folder it left: `=0` the current one, `=-` the last,
//!   `=N` past the stack `Directory stack not that deep.`; only as the word or before a `/`),
//!   then a pattern (`*`, `?`, `[...]` with a closing `]`: `^` negates, `a-z` ranges, `!` is a
//!   character) matched against the files, in byte order; a name starting with `.` matched only by
//!   a pattern starting with `.` (`.*` matches `.` and `..`). In one command's arguments, patterns
//!   that match nothing are dropped when another matched, else the command fails with
//!   `<command>: No match.` (`set: No match.`).
//! - **`echo`** (`echo_style` both): a first word `-n` drops the new line; `\a \b \c \e \f \n \r
//!   \t \v \\` and `\0` with up to three octal digits are their characters (`\c` ends the output
//!   there, without a new line); any other backslash stays.
//! - **Command substitution** (`` `cmd` ``): the output's words, split at blanks and new lines,
//!   each given its braces, `~` and `=N` when `set` assigns them, and matched as patterns only when
//!   the command's own text (its variables substituted) holds `*`, `?` or `[` (`` `pwd` ``'s path is
//!   never matched). A command substitution's own patterns that match nothing: `<command>: No
//!   match.` from it, and no output.

use std::path::{Path, PathBuf};

use larmorx_freesurfer_core::run::Environment;

/// A message of tcsh's (or of a program it ran) ending the script, status 1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fail {
    /// The lines on standard error, each with its new line.
    pub stderr: String,
}

impl Fail {
    pub fn new(line: &str) -> Fail {
        Fail {
            stderr: format!("{line}\n"),
        }
    }
}

/// The shell's state: its environment, current folder and directory stack.
pub struct Shell<'a> {
    pub env: &'a Environment,
    /// The current folder (absolute).
    pub cwd: PathBuf,
    /// The directory stack after the current folder (`pushd` adds the folder it left).
    pub stack: Vec<PathBuf>,
}

/// A word split at blanks (space, tab), empty pieces dropped.
pub fn split_blanks(w: &str) -> Vec<String> {
    w.split([' ', '\t'])
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// A command substitution's output split into words at blanks and new lines.
pub fn split_output(s: &str) -> Vec<String> {
    s.split([' ', '\t', '\n'])
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Brace expansion of one word (the words `{`, `}` and `{}` stay as they are; `{}` inside a word
/// is an empty choice: `a{}b` is `ab`).
pub fn braces(w: &str) -> Result<Vec<String>, Fail> {
    if matches!(w, "{" | "}" | "{}") {
        return Ok(vec![w.to_owned()]);
    }
    let b: Vec<char> = w.chars().collect();
    let mut i = 0;
    while i < b.len() {
        if b[i] == '{' {
            // the matching '}'
            let (mut depth, mut j) = (0usize, i);
            let mut close = None;
            while j < b.len() {
                match b[j] {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            close = Some(j);
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            let Some(close) = close else {
                return Err(Fail::new("Missing '}'."));
            };
            let prefix: String = b[..i].iter().collect();
            let suffix: String = b[close + 1..].iter().collect();
            let mut alts = Vec::new();
            let (mut depth, mut start) = (0usize, i + 1);
            for k in i + 1..close {
                match b[k] {
                    '{' => depth += 1,
                    '}' => depth -= 1,
                    ',' if depth == 0 => {
                        alts.push(b[start..k].iter().collect::<String>());
                        start = k + 1;
                    }
                    _ => {}
                }
            }
            alts.push(b[start..close].iter().collect::<String>());
            let mut out = Vec::new();
            for alt in alts {
                out.extend(braces(&format!("{prefix}{alt}{suffix}"))?);
            }
            return Ok(out);
        }
        i += 1;
    }
    Ok(vec![w.to_owned()])
}

/// A user's home folder from the password file (`/etc/passwd`).
fn passwd_home(user: &str) -> Option<String> {
    let text = std::fs::read_to_string("/etc/passwd").ok()?;
    text.lines().find_map(|l| {
        let f: Vec<&str> = l.split(':').collect();
        (f.len() >= 7 && f[0] == user).then(|| f[5].to_owned())
    })
}

/// Whether a word holds a bracket range (`[a-c]`), whose matching in tcsh 6.21 this model does
/// not reproduce (measured: `[a-c]` matches `0`, `A`, `Z`, `_` as well, not as the characters'
/// codes order them).
pub fn has_range(w: &str) -> bool {
    let b = w.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'[' {
            if let Some(k) = b[i + 1..].iter().position(|&c| c == b']') {
                let inner = &b[i + 1..i + 1 + k.max(1)];
                let inner = inner.strip_prefix(b"^").unwrap_or(inner);
                if inner.len() >= 3 && inner[1..inner.len() - 1].contains(&b'-') {
                    return true;
                }
                i += k + 2;
                continue;
            }
        }
        i += 1;
    }
    false
}

/// Whether a word holds a pattern: `*`, `?`, or a `[` with a `]` after it.
pub fn has_pattern(w: &str) -> bool {
    let b = w.as_bytes();
    b.iter()
        .enumerate()
        .any(|(i, &c)| c == b'*' || c == b'?' || (c == b'[' && b[i + 1..].contains(&b']')))
}

/// One name against one pattern piece (no `/`).
fn matches(p: &[char], n: &[char]) -> bool {
    if p.is_empty() {
        return n.is_empty();
    }
    match p[0] {
        '*' => (0..=n.len()).any(|k| matches(&p[1..], &n[k..])),
        '?' => !n.is_empty() && matches(&p[1..], &n[1..]),
        '[' if p[1..].contains(&']') => {
            let Some(&c) = n.first() else {
                return false;
            };
            let mut k = 1;
            let negate = p.get(1) == Some(&'^');
            if negate {
                k += 1;
            }
            let mut hit = false;
            let mut first = true;
            while k < p.len() && (p[k] != ']' || first) {
                first = false;
                if p.get(k + 1) == Some(&'-') && p.get(k + 2).is_some_and(|&e| e != ']') {
                    if p[k] <= c && c <= p[k + 2] {
                        hit = true;
                    }
                    k += 3;
                } else {
                    if p[k] == c {
                        hit = true;
                    }
                    k += 1;
                }
            }
            if k >= p.len() {
                return false;
            }
            hit != negate && matches(&p[k + 1..], &n[1..])
        }
        x => n.first() == Some(&x) && matches(&p[1..], &n[1..]),
    }
}

impl Shell<'_> {
    fn at(&self, p: &str) -> PathBuf {
        if p.starts_with('/') {
            PathBuf::from(p)
        } else {
            self.cwd.join(p)
        }
    }

    /// A pattern's matches, in byte order (none: empty).
    fn glob(&self, w: &str) -> Vec<String> {
        let absolute = w.starts_with('/');
        let parts: Vec<&str> = w.split('/').collect();
        let mut found: Vec<String> = vec![if absolute { "/".into() } else { String::new() }];
        let pieces = if absolute { &parts[1..] } else { &parts[..] };
        for (k, piece) in pieces.iter().enumerate() {
            let last = k + 1 == pieces.len();
            let mut next = Vec::new();
            for pre in &found {
                let join = |name: &str| {
                    if pre.is_empty() || pre.ends_with('/') {
                        format!("{pre}{name}")
                    } else {
                        format!("{pre}/{name}")
                    }
                };
                if piece.is_empty() {
                    // `a//b` or a final `/`: kept, the name so far must be a folder
                    if pre.is_empty() || self.at(pre).is_dir() {
                        next.push(format!("{pre}/"));
                    }
                    continue;
                }
                if !has_pattern(piece) {
                    let p = join(piece);
                    if last || self.at(&p).is_dir() {
                        next.push(p);
                    }
                    continue;
                }
                let dir = if pre.is_empty() {
                    self.cwd.clone()
                } else {
                    self.at(pre)
                };
                let Ok(rd) = std::fs::read_dir(&dir) else {
                    continue;
                };
                let mut names: Vec<String> = rd
                    .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
                    .collect();
                names.push(".".into());
                names.push("..".into());
                let pc: Vec<char> = piece.chars().collect();
                for n in names {
                    if n.starts_with('.') && !piece.starts_with('.') {
                        continue;
                    }
                    let nc: Vec<char> = n.chars().collect();
                    if matches(&pc, &nc) && (last || dir.join(&n).is_dir()) {
                        next.push(join(&n));
                    }
                }
            }
            found = next;
        }
        // the last piece's existence (a literal last piece after a pattern)
        let mut out: Vec<String> = found
            .into_iter()
            .filter(|p| {
                std::fs::symlink_metadata(self.at(p.trim_end_matches('/'))).is_ok() || p == "/"
            })
            .collect();
        out.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
        out
    }

    /// One piece's braces, `~`, `=N` and pattern: its words and whether it held a pattern that
    /// matched nothing (`Some(false)`), matched (`Some(true)`) or none (`None`).
    pub fn expand_piece(&self, w: &str) -> Result<(Vec<String>, Option<bool>), Fail> {
        self.expand_piece_with(w, true)
    }

    /// [`Shell::expand_piece`], matching patterns only when `glob` says so.
    pub fn expand_piece_with(
        &self,
        w: &str,
        glob: bool,
    ) -> Result<(Vec<String>, Option<bool>), Fail> {
        let mut words = Vec::new();
        let mut state: Option<bool> = None;
        for b in braces(w)? {
            let mut b = b;
            if let Some(rest) = b.strip_prefix('~') {
                let (user, tail) = match rest.find('/') {
                    Some(k) => (&rest[..k], &rest[k..]),
                    None => (rest, ""),
                };
                let home = if user.is_empty() {
                    self.env
                        .get("HOME")
                        .map(|h| h.to_string_lossy().into_owned())
                        .ok_or_else(|| Fail::new("No $home variable set."))?
                } else {
                    passwd_home(user).ok_or_else(|| Fail::new(&format!("Unknown user: {user}.")))?
                };
                b = format!("{home}{tail}");
            } else if let Some(rest) = b.strip_prefix('=') {
                // `=N` and `=-` only as the word or before a `/`
                let digits = rest.chars().take_while(char::is_ascii_digit).count();
                let len = if rest.starts_with('-') { 1 } else { digits };
                let whole = len > 0 && (rest.len() == len || rest[len..].starts_with('/'));
                if whole {
                    let depth = 1 + self.stack.len();
                    let (n, tail) = if rest.starts_with('-') {
                        (depth - 1, &rest[1..])
                    } else {
                        (
                            rest[..digits].parse().unwrap_or(usize::MAX),
                            &rest[digits..],
                        )
                    };
                    let entry = match n {
                        0 => &self.cwd,
                        n if n < depth => &self.stack[n - 1],
                        _ => return Err(Fail::new("Directory stack not that deep.")),
                    };
                    b = format!("{}{tail}", entry.to_string_lossy());
                }
            }
            if glob && has_pattern(&b) {
                let m = self.glob(&b);
                state = Some(state.unwrap_or(false) || !m.is_empty());
                words.extend(m);
            } else {
                words.push(b);
            }
        }
        Ok((words, state))
    }

    /// A command's arguments from unquoted words: each split at blanks and expanded; `cmd: No
    /// match.` (or `No match.` without a command) when patterns were given and none matched.
    pub fn args(&self, cmd: &str, words: &[String]) -> Result<Vec<String>, Fail> {
        let mut out = Vec::new();
        let (mut patterns, mut matched) = (false, false);
        for w in words {
            for piece in split_blanks(w) {
                let (ws, st) = self.expand_piece(&piece)?;
                if let Some(m) = st {
                    patterns = true;
                    matched |= m;
                }
                out.extend(ws);
            }
        }
        if patterns && !matched {
            return Err(Fail::new(&if cmd.is_empty() {
                "No match.".to_owned()
            } else {
                format!("{cmd}: No match.")
            }));
        }
        Ok(out)
    }
}

/// tcsh's `echo` of its words (`echo_style` both): the bytes it writes.
pub fn echo(words: &[String]) -> Vec<u8> {
    let (words, newline) = match words.first() {
        Some(w) if w == "-n" => (&words[1..], false),
        _ => (words, true),
    };
    let text = words.join(" ");
    let b = text.as_bytes();
    let mut out = Vec::with_capacity(b.len() + 1);
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' && i + 1 < b.len() {
            let c = b[i + 1];
            let mapped = match c {
                b'a' => Some(7u8),
                b'b' => Some(8),
                b'e' => Some(27),
                b'f' => Some(12),
                b'n' => Some(b'\n'),
                b'r' => Some(b'\r'),
                b't' => Some(b'\t'),
                b'v' => Some(11),
                b'\\' => Some(b'\\'),
                _ => None,
            };
            if let Some(m) = mapped {
                out.push(m);
                i += 2;
                continue;
            }
            if c == b'c' {
                return out;
            }
            if c == b'0' {
                let mut v: u32 = 0;
                let mut k = i + 2;
                while k < b.len() && k < i + 5 && (b'0'..=b'7').contains(&b[k]) {
                    v = v * 8 + u32::from(b[k] - b'0');
                    k += 1;
                }
                out.push((v & 0xff) as u8);
                i = k;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    if newline {
        out.push(b'\n');
    }
    out
}

/// GNU `dirname` of one operand.
pub fn dirname(s: &str) -> String {
    dirname_on(s, false)
}

/// GNU `basename` of one operand.
pub fn basename(s: &str) -> String {
    basename_on(s, false)
}

/// [`dirname`] with `\` a separator as well (`windows`: the port's rule on Windows).
pub fn dirname_on(s: &str, windows: bool) -> String {
    let sep = |c: char| c == '/' || (windows && c == '\\');
    let b = s.trim_end_matches(sep);
    if b.is_empty() {
        return if s.starts_with(sep) { "/" } else { "." }.to_owned();
    }
    match b.rfind(sep) {
        None => ".".to_owned(),
        Some(k) => {
            let d = b[..k].trim_end_matches(sep);
            if d.is_empty() { &b[..1] } else { d }.to_owned()
        }
    }
}

/// [`basename`] with `\` a separator as well (`windows`).
pub fn basename_on(s: &str, windows: bool) -> String {
    let sep = |c: char| c == '/' || (windows && c == '\\');
    let b = s.trim_end_matches(sep);
    if b.is_empty() {
        return if s.is_empty() { "" } else { &s[..1] }.to_owned();
    }
    match b.rfind(sep) {
        None => b.to_owned(),
        Some(k) => b[k + 1..].to_owned(),
    }
}

/// The message GNU `dirname`'s options give for operands starting with `-` (other than `-`
/// alone), with its standard output: `Ok(None)` when there are none (the operands are plain),
/// `Err` the error text (status 1) or `Ok(Some(..))` a form this model does not reproduce.
pub fn coreutils_options(prog: &str, args: &[String]) -> Result<Option<&'static str>, String> {
    let try_help = format!("Try '{prog} --help' for more information.\n");
    let mut operands = 0usize;
    let mut end = false;
    let mut zero = false;
    for a in args {
        if end || a == "-" || !a.starts_with('-') {
            operands += 1;
            continue;
        }
        if a == "--" {
            end = true;
            continue;
        }
        if let Some(long) = a.strip_prefix("--") {
            let name = long.split('=').next().unwrap_or("");
            let known = ["help", "version", "zero"];
            let hits: Vec<&str> = known
                .iter()
                .copied()
                .filter(|k| k.starts_with(name))
                .collect();
            match hits.as_slice() {
                [] => return Err(format!("{prog}: unrecognized option '{a}'\n{try_help}")),
                ["help"] => return Ok(Some(HELP)),
                ["version"] => return Ok(Some(VERSION)),
                ["zero"] => {
                    zero = true;
                    continue;
                }
                _ => return Ok(Some("an ambiguous long option")),
            }
        }
        for c in a[1..].chars() {
            if c != 'z' {
                return Err(format!("{prog}: invalid option -- '{c}'\n{try_help}"));
            }
        }
        zero = true;
    }
    if operands == 0 {
        return Err(format!("{prog}: missing operand\n{try_help}"));
    }
    if zero {
        return Ok(Some("-z"));
    }
    Ok(None)
}

/// [`coreutils_options`]'s answer for `--help`: the program prints its help (many words).
pub const HELP: &str = "--help";
/// [`coreutils_options`]'s answer for `--version`: the program prints its version (many words).
pub const VERSION: &str = "--version";

/// Whether `dir` (as written, from `cwd`) can be entered (`cd`): the message tcsh's `pushd`
/// gives when it cannot (`<dir>: Not a directory.`, `<dir>: Permission denied.`, `<dir>: No such
/// file or directory.`).
pub fn enter(cwd: &Path, dir: &str) -> Result<PathBuf, String> {
    let p = if dir.starts_with('/') {
        PathBuf::from(dir)
    } else {
        cwd.join(dir)
    };
    match std::fs::metadata(&p) {
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            return Err(format!("{dir}: Permission denied."));
        }
        Err(_) => return Err(format!("{dir}: No such file or directory.")),
        Ok(m) if !m.is_dir() => return Err(format!("{dir}: Not a directory.")),
        Ok(_) => {}
    }
    // entering needs search permission: `<dir>/.` cannot be looked at without it
    if let Err(e) = std::fs::metadata(p.join(".")) {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            return Err(format!("{dir}: Permission denied."));
        }
    }
    match physical(&p) {
        Some(s) => Ok(PathBuf::from(s)),
        None => Err(format!("{dir}: No such file or directory.")),
    }
}

/// The physical name of a folder (links resolved, as `/bin/pwd` prints it); on Windows without
/// the verbatim prefix (`\\?\`).
pub fn physical(p: &Path) -> Option<String> {
    let c = std::fs::canonicalize(p).ok()?;
    let s = c.to_string_lossy().into_owned();
    Some(if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        rest.to_owned()
    } else {
        s
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn braces_expand_as_tcsh() {
        assert_eq!(braces("a{b,c}d").unwrap(), ["abd", "acd"]);
        assert_eq!(braces("{}").unwrap(), ["{}"]);
        assert_eq!(braces("a{}b").unwrap(), ["ab"]);
        assert_eq!(braces("x/{}").unwrap(), ["x/"]);
        assert_eq!(braces("x}").unwrap(), ["x}"]);
        assert_eq!(braces("x{a}").unwrap(), ["xa"]);
        assert_eq!(braces("{,}").unwrap(), ["", ""]);
        assert!(braces("a{b,c").is_err());
        assert_eq!(braces("{a,{b,c}}").unwrap(), ["a", "b", "c"]);
    }

    #[test]
    fn echo_escapes() {
        let w = |s: &str| vec![s.to_owned()];
        assert_eq!(echo(&w("a\\tb")), b"a\tb\n");
        assert_eq!(echo(&w("a\\c")), b"a");
        assert_eq!(echo(&w("a\\0141\\0142")), b"aab\n");
        assert_eq!(echo(&w("a\\08b")), b"a\x008b\n");
        assert_eq!(echo(&w("a\\101b")), b"a\\101b\n");
        assert_eq!(echo(&w("a\\")), b"a\\\n");
        assert_eq!(echo(&w("-n")), b"");
    }

    #[test]
    fn dirname_and_basename_as_coreutils() {
        for (s, d, b) in [
            ("/", "/", "/"),
            ("//", "/", "/"),
            ("/.", "/", "."),
            ("///x", "/", "x"),
            ("a//", ".", "a"),
            ("d1/", ".", "d1"),
            ("d1//x", "d1", "x"),
            ("a/b/", "a", "b"),
            ("x", ".", "x"),
        ] {
            assert_eq!((dirname(s).as_str(), basename(s).as_str()), (d, b), "{s}");
        }
    }

    #[test]
    fn patterns() {
        let c = |s: &str| s.chars().collect::<Vec<_>>();
        assert!(matches(&c("[^x]*"), &c("y.mgz")));
        assert!(!matches(&c("[^x]*"), &c("x.mgz")));
        assert!(matches(&c("[!x]*"), &c("x.mgz")));
        assert!(matches(&c("[a-]"), &c("a")));
        assert!(matches(&c("[a-y]*"), &c("x")));
        assert!(has_pattern("a*") && !has_pattern("x[") && has_pattern("x[a]"));
    }
}
