// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-flirt 2111.4 (the scripts pairreg, extracttxt,
// aff2rigid; flirtsch/pairreg1.sch, pairreg2.sch, pairreg3.sch). Copyright (C) 1999-2023
// University of Oxford (Mark Jenkinson). Rust translation Copyright 2026 Karel Lopez Vilaret;
// modified from the originals. Distributed under the FSL Licence (see LICENSE).
//! FLIRT's scripts whose programs the replica has: `pairreg` (three FLIRT runs with FSL's
//! `pairreg*.sch` schedules and `convert_xfm -fixscaleskew`, run in-process), `extracttxt` (the
//! lines of a file from the first that matches a word, as `grep`, `dc` and `sed` give them) and
//! `aff2rigid` (the 6-parameter approximation of a 12-parameter standard-space matrix, as numpy
//! computes it on FSL's OpenBLAS). Each prints the script's messages and exits as it does; the
//! programs it calls are the replica's own.

use std::io::Write;

use larmorx_core::defects::{self, Notice, UpstreamDefects};
use larmorx_nc_core::lapack;
use larmorx_nc_core::lapack::blas::{self, MatRef};
use larmorx_nc_core::settings::Settings;

use crate::run::PREFIX;

const PAIRREG1: &str = include_str!("../../fsl-source/fsl-flirt/flirtsch/pairreg1.sch");
const PAIRREG2: &str = include_str!("../../fsl-source/fsl-flirt/flirtsch/pairreg2.sch");
const PAIRREG3: &str = include_str!("../../fsl-source/fsl-flirt/flirtsch/pairreg3.sch");

/// The shell's word splitting of an unquoted `$var` (IFS: space, tab, newline); words with the
/// pathname-expansion characters `*`, `?`, `[` are refused (`Err`): the shell would expand them.
fn split_words(v: &str) -> Result<Vec<String>, String> {
    let words: Vec<String> = v
        .split([' ', '\t', '\n'])
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect();
    if let Some(w) = words.iter().find(|w| w.contains(['*', '?', '['])) {
        return Err(format!(
            "the word {w:?}: the script's unquoted variable would be expanded by the shell as a \
             file name pattern"
        ));
    }
    Ok(words)
}

fn refuse(err: &mut dyn Write, what: &str) -> u8 {
    let _ = writeln!(err, "{PREFIX}: error: {what}");
    1
}

/// `pairreg brain1 brain2 skull1 skull2 outputmatrix [extra flirt args]`.
pub fn pairreg(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    if args.len() < 5 {
        let _ = writeln!(
            out,
            "Usage: pairreg brain1 brain2 skull1 skull2 outputmatrix [extra flirt args]"
        );
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "  NB: brain1 and skull1 are used as -ref arguments internally"
        );
        return 1;
    }
    let words = |v: &str| split_words(v);
    let split = (|| -> Result<_, String> {
        Ok((
            words(&args[0])?,
            words(&args[1])?,
            words(&args[2])?,
            words(&args[3])?,
            words(&args[4])?,
            words(&args[5..].join(" "))?,
        ))
    })();
    let (bref, bin, sref, sin, matf, extra) = match split {
        Ok(v) => v,
        Err(e) => return refuse(err, &e),
    };
    let matf_s = args[4].clone();
    let pid = std::process::id();
    let tmp = |k: u32| format!("{matf_s}.{k}.{pid}");
    // The schedules, FSL's files, in a private directory for the run.
    let dir = std::env::temp_dir().join(format!("larmorx-pairreg-{pid}"));
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return refuse(err, &format!("cannot make a temporary directory: {e}"));
    }
    let sch = |k: u32, text: &str| -> String {
        let p = dir.join(format!("pairreg{k}.sch"));
        let _ = std::fs::write(&p, text);
        p.to_string_lossy().into_owned()
    };
    let (s1, s2, s3) = (sch(1, PAIRREG1), sch(2, PAIRREG2), sch(3, PAIRREG3));
    let cmd = |parts: Vec<Vec<String>>| -> Vec<String> { parts.into_iter().flatten().collect() };
    let one = |s: &str| vec![s.to_owned()];
    // $flirt -in $bin -ref $bref -schedule .../pairreg1.sch -omat $matf.1.$$ $args
    let a1 = cmd(vec![
        one("-in"),
        bin.clone(),
        one("-ref"),
        bref.clone(),
        one("-schedule"),
        one(&s1),
        one("-omat"),
        split_words(&tmp(1)).unwrap_or_default(),
        extra.clone(),
    ]);
    let _ = crate::flirt(&a1, settings, mode, threads, out, err);
    let a2 = cmd(vec![
        one("-in"),
        sin,
        one("-ref"),
        sref,
        one("-init"),
        split_words(&tmp(1)).unwrap_or_default(),
        one("-schedule"),
        one(&s2),
        one("-omat"),
        split_words(&tmp(2)).unwrap_or_default(),
        extra.clone(),
    ]);
    let _ = crate::flirt(&a2, settings, mode, threads, out, err);
    let a3 = cmd(vec![
        one("-fixscaleskew"),
        split_words(&tmp(2)).unwrap_or_default(),
        split_words(&tmp(1)).unwrap_or_default(),
        one("-omat"),
        split_words(&tmp(3)).unwrap_or_default(),
    ]);
    let _ = crate::convert_xfm(&a3, settings, mode, threads, out, err);
    let a4 = cmd(vec![
        one("-in"),
        bin,
        one("-ref"),
        bref,
        one("-init"),
        split_words(&tmp(3)).unwrap_or_default(),
        one("-schedule"),
        one(&s3),
        one("-omat"),
        matf,
        extra,
    ]);
    let _ = crate::flirt(&a4, settings, mode, threads, out, err);
    // rm -f $matf.1.$$ $matf.2.$$ $matf.3.$$ (the script's status: 0)
    for k in 1..=3 {
        let _ = std::fs::remove_file(tmp(k));
    }
    let _ = std::fs::remove_dir_all(&dir);
    0
}

// ------------------------------------------------------------------------------------------------
// extracttxt

/// One element of a basic regular expression (POSIX BRE as GNU grep reads it in the C locale,
/// without its extensions `\(`, `\{`, `\|`, `\+`, `\?`, back-references and classes).
#[derive(Clone, Debug)]
enum Atom {
    Byte(u8),
    Any,
    Set(Vec<(u8, u8)>, bool),
}

#[derive(Clone, Debug)]
struct Piece {
    atom: Atom,
    star: bool,
}

struct Regex {
    anchored_start: bool,
    anchored_end: bool,
    pieces: Vec<Piece>,
}

fn parse_bre(p: &[u8]) -> Result<Regex, String> {
    let mut i = 0;
    let mut anchored_start = false;
    if p.first() == Some(&b'^') {
        anchored_start = true;
        i = 1;
    }
    let mut end = p.len();
    let mut anchored_end = false;
    if end > i && p[end - 1] == b'$' && (end < 2 || p[end - 2] != b'\\') {
        anchored_end = true;
        end -= 1;
    }
    let mut pieces: Vec<Piece> = Vec::new();
    while i < end {
        let c = p[i];
        let atom = match c {
            b'.' => {
                i += 1;
                Atom::Any
            }
            b'*' if pieces.is_empty() => {
                // a leading star is literal
                i += 1;
                Atom::Byte(b'*')
            }
            b'*' => {
                if let Some(last) = pieces.last_mut() {
                    last.star = true;
                }
                i += 1;
                continue;
            }
            b'[' => {
                let mut j = i + 1;
                let neg = p.get(j) == Some(&b'^');
                if neg {
                    j += 1;
                }
                let mut set = Vec::new();
                let mut first = true;
                loop {
                    let Some(&b) = p.get(j) else {
                        return Err("an unmatched [".to_owned());
                    };
                    if b == b']' && !first {
                        break;
                    }
                    if b == b'[' && matches!(p.get(j + 1), Some(b':' | b'.' | b'=')) {
                        return Err("a bracket class ([: :], [. .], [= =])".to_owned());
                    }
                    if p.get(j + 1) == Some(&b'-') && p.get(j + 2).is_some_and(|&e| e != b']') {
                        set.push((b, p[j + 2]));
                        j += 3;
                    } else {
                        set.push((b, b));
                        j += 1;
                    }
                    first = false;
                }
                i = j + 1;
                Atom::Set(set, neg)
            }
            b'\\' => {
                let Some(&n) = p.get(i + 1) else {
                    return Err("a trailing backslash".to_owned());
                };
                if matches!(
                    n,
                    b'(' | b')'
                        | b'{'
                        | b'}'
                        | b'|'
                        | b'+'
                        | b'?'
                        | b'<'
                        | b'>'
                        | b'b'
                        | b'B'
                        | b'w'
                        | b'W'
                        | b's'
                        | b'S'
                        | b'`'
                        | b'\''
                ) || n.is_ascii_digit()
                {
                    return Err(format!("the extension \\{}", char::from(n)));
                }
                i += 2;
                Atom::Byte(n)
            }
            _ => {
                i += 1;
                Atom::Byte(c)
            }
        };
        pieces.push(Piece { atom, star: false });
    }
    Ok(Regex {
        anchored_start,
        anchored_end,
        pieces,
    })
}

fn atom_matches(a: &Atom, b: u8) -> bool {
    match a {
        Atom::Byte(x) => *x == b,
        Atom::Any => true,
        Atom::Set(set, neg) => set.iter().any(|&(lo, hi)| b >= lo && b <= hi) != *neg,
    }
}

fn match_here(pieces: &[Piece], s: &[u8], end_anchor: bool) -> bool {
    let Some((first, rest)) = pieces.split_first() else {
        return !end_anchor || s.is_empty();
    };
    if first.star {
        let mut k = 0;
        loop {
            if match_here(rest, &s[k..], end_anchor) {
                return true;
            }
            if k < s.len() && atom_matches(&first.atom, s[k]) {
                k += 1;
            } else {
                return false;
            }
        }
    }
    !s.is_empty() && atom_matches(&first.atom, s[0]) && match_here(rest, &s[1..], end_anchor)
}

impl Regex {
    fn is_match(&self, line: &[u8]) -> bool {
        if self.anchored_start {
            return match_here(&self.pieces, line, self.anchored_end);
        }
        (0..=line.len()).any(|k| match_here(&self.pieces, &line[k..], self.anchored_end))
    }
}

/// A shell integer argument of `[` and `dc` (an optional sign and decimal digits).
fn integer(s: &str) -> Option<i64> {
    let t = s.strip_prefix(['+', '-']).unwrap_or(s);
    if t.is_empty() || !t.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let v: i64 = t.parse().ok()?;
    // dc's results stay far inside i64 (a line number plus two arguments)
    if v > i64::MAX / 4 {
        return None;
    }
    Some(if s.starts_with('-') { -v } else { v })
}

/// GNU dc 1.4.1 on `expr` (numbers, `+`, `-`, `p`, blanks): a `+` or `-` that finds fewer
/// than two numbers on the stack writes "dc: stack empty" and does nothing, so the sign of a
/// signed number is a command and its digits a separate number. Returns the number `p`
/// prints (dc prints a negative one as `-N`).
fn dc(expr: &str, err: &mut dyn Write) -> Option<i64> {
    let mut stack: Vec<i64> = Vec::new();
    let mut printed = None;
    let b = expr.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'0'..=b'9' => {
                let start = i;
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
                stack.push(expr[start..i].parse().ok()?);
                continue;
            }
            c @ (b'+' | b'-') => {
                if stack.len() < 2 {
                    let _ = writeln!(err, "dc: stack empty");
                } else {
                    let y = stack.pop()?;
                    let x = stack.pop()?;
                    stack.push(if c == b'+' { x + y } else { x - y });
                }
            }
            b'p' => printed = stack.last().copied(),
            _ => {}
        }
        i += 1;
    }
    printed
}

/// GNU sed 4.8's usage text after an unknown option (on standard error, exit status 1).
const SED_USAGE: &str = "Usage: sed [OPTION]... {script-only-if-no-other-script} [input-file]...

  -n, --quiet, --silent
                 suppress automatic printing of pattern space
      --debug
                 annotate program execution
  -e script, --expression=script
                 add the script to the commands to be executed
  -f script-file, --file=script-file
                 add the contents of script-file to the commands to be executed
  --follow-symlinks
                 follow symlinks when processing in place
  -i[SUFFIX], --in-place[=SUFFIX]
                 edit files in place (makes backup if SUFFIX supplied)
  -l N, --line-length=N
                 specify the desired line-wrap length for the `l' command
  --posix
                 disable all GNU extensions.
  -E, -r, --regexp-extended
                 use extended regular expressions in the script
                 (for portability use POSIX -E).
  -s, --separate
                 consider files as separate rather than as a single,
                 continuous long stream.
      --sandbox
                 operate in sandbox mode (disable e/r/w commands).
  -u, --unbuffered
                 load minimal amounts of data from the input files and flush
                 the output buffers more often
  -z, --null-data
                 separate lines by NUL characters
      --help     display this help and exit
      --version  output version information and exit

If no -e, --expression, -f, or --file option is given, then the first
non-option argument is taken as the sed script to interpret.  All
remaining arguments are names of input files; if no input files are
specified, then the standard input is read.

GNU sed home page: <https://www.gnu.org/software/sed/>.
General help using GNU software: <https://www.gnu.org/gethelp/>.
";

/// `extracttxt search_word file [num_trailing_lines [relative_start]]`.
pub fn extracttxt(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let _ = (settings, mode, threads);
    if args.len() <= 1 {
        let _ = writeln!(
            out,
            "Usage: extracttxt search_word file [num_trailing_lines [relative_start]]"
        );
        // `exit -1` under /bin/sh (dash): an illegal number, exit status 2.
        let _ = writeln!(err, "extracttxt: 68: exit: Illegal number: -1");
        return 2;
    }
    let (word, file) = (&args[0], &args[1]);
    if word.is_empty()
        || file.is_empty()
        || word.contains([' ', '\t', '\n'])
        || file.contains([' ', '\t', '\n'])
    {
        return refuse(
            err,
            "an empty word or one with blanks: the script passes $1 and $2 to grep unquoted",
        );
    }
    if word.starts_with('-') {
        return refuse(
            err,
            "a search word starting with '-': grep reads it as an option",
        );
    }
    let re = match parse_bre(word.as_bytes()) {
        Ok(r) => r,
        Err(e) => {
            return refuse(
                err,
                &format!("the search word's regular expression has {e}"),
            );
        }
    };
    let n3 = args.get(2).map(|s| (s, integer(s)));
    let n4 = args.get(3).map(|s| (s, integer(s)));
    if n3.as_ref().is_some_and(|(_, v)| v.is_none())
        || n4.as_ref().is_some_and(|(_, v)| v.is_none())
    {
        return refuse(
            err,
            "num_trailing_lines and relative_start must be integers",
        );
    }
    let text = match std::fs::read(file) {
        Ok(t) => t,
        Err(_) => {
            let _ = writeln!(err, "grep: {file}: No such file or directory");
            return 0;
        }
    };
    if text.contains(&0u8) {
        return refuse(
            err,
            "a binary file (grep reports a match without its line number)",
        );
    }
    let mut lines: Vec<&[u8]> = text.split(|&b| b == b'\n').collect();
    if text.ends_with(b"\n") {
        lines.pop();
    }
    let Some(num) = lines
        .iter()
        .position(|l| re.is_match(l))
        .map(|k| k as i64 + 1)
    else {
        return 0;
    };
    // the script's arithmetic, in dc on the arguments as given
    let mut num1 = match n4 {
        Some((s4, Some(v4))) => {
            let expr = if v4 >= 0 {
                format!("{num} {s4} + p")
            } else {
                format!("0 0 {s4} - {num} + p")
            };
            match dc(&expr, err) {
                Some(v) => v,
                None => return refuse(err, "relative_start out of range"),
            }
        }
        _ => num,
    };
    let num2: Option<i64> = match n3 {
        Some((s3, Some(_))) => {
            // dc prints a negative num1 as `-N`, and reading it back takes the `-` for its
            // subtraction command
            let Some(n2) = dc(&format!("{num1} {s3} + 1 - p"), err) else {
                return refuse(err, "num_trailing_lines out of range");
            };
            if n2 < num1 {
                num1 = n2;
            }
            Some(n2)
        }
        _ => None,
    };
    // sed -n "$num1,$num2"p $2
    let expr = format!("{num1},{}p", num2.map_or("$".to_owned(), |v| v.to_string()));
    if num1 < 0 {
        // the range starts with `-`: sed reads it as options, its first digit an unknown one
        let digit = expr.chars().nth(1).unwrap_or('-');
        let _ = writeln!(err, "sed: invalid option -- '{digit}'");
        let _ = err.write_all(SED_USAGE.as_bytes());
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
    let first = num1 as usize;
    let last = num2.map_or(usize::MAX, |v| (v.max(0) as usize).max(first));
    let mut o = Vec::new();
    for (k, l) in lines.iter().enumerate() {
        let n = k + 1;
        if n >= first && n <= last {
            o.extend_from_slice(l);
            o.push(b'\n');
        }
    }
    // the file's last line without its newline is printed as sed prints it
    if !text.ends_with(b"\n") && last >= lines.len() && first <= lines.len() {
        o.pop();
    }
    let _ = out.write_all(&o);
    0
}

// ------------------------------------------------------------------------------------------------
// aff2rigid

/// numpy's `x.dot(x)` of a 4-vector (`cblas_ddot`: OpenBLAS's unit-stride kernel).
fn dot4(x: &[f64; 4], y: &[f64; 4]) -> f64 {
    blas::ddot(x, 1, y, 1)
}

/// `linalg.norm(v)` of a 4 × 1 matrix: `sqrt(x.dot(x))` of its values.
fn norm4(v: &[f64; 4]) -> f64 {
    dot4(v, v).sqrt()
}

/// numpy's `A @ x` for a C-ordered 4 × 4 `A` (`cblas_dgemv` with `CblasRowMajor`: OpenBLAS's
/// transposed kernel on the column-major view).
fn gemv4(a: &[[f64; 4]; 4], x: &[f64; 4]) -> [f64; 4] {
    // The column-major view of a row-major A is Aᵀ: element (r, c) of the view is a[c][r].
    let mut view = [0.0f64; 16];
    for i in 0..4 {
        for j in 0..4 {
            view[i * 4 + j] = a[i][j];
        }
    }
    let mut y = [0.0f64; 4];
    blas::dgemv_t(
        4,
        4,
        1.0,
        &view,
        MatRef { off: 0, ld: 4 },
        x,
        1,
        0.0,
        &mut y,
    );
    y
}

/// numpy's `A @ B` of 4 × 4 matrices (`cblas_dgemm`: each element the kernel's FMA chain).
fn gemm4(a: &[[f64; 4]; 4], b: &[[f64; 4]; 4]) -> [[f64; 4]; 4] {
    let mut am = [0.0f64; 16];
    let mut bm = [0.0f64; 16];
    for i in 0..4 {
        for j in 0..4 {
            am[j * 4 + i] = a[i][j];
            bm[j * 4 + i] = b[i][j];
        }
    }
    let mut c = [0.0f64; 16];
    let m = MatRef { off: 0, ld: 4 };
    blas::dgemm(false, false, 4, 4, 4, 1.0, &am, m, &bm, m, 0.0, &mut c, m);
    let mut out = [[0.0f64; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            out[i][j] = c[j * 4 + i];
        }
    }
    out
}

/// numpy's `cross` of 3-vectors: each component a rounded product less another.
fn cross3(a: &[f64], b: &[f64]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Python's `'%14.10f' % v`.
fn py_f(v: f64) -> String {
    if v.is_nan() {
        return format!("{:>14}", "nan");
    }
    if v.is_infinite() {
        return format!("{:>14}", if v > 0.0 { "inf" } else { "-inf" });
    }
    format!("{v:14.10}")
}

/// `numpy.loadtxt(name)` of a 4 × 4 matrix (blank-separated decimal numbers, `#` comments).
fn loadtxt4(name: &str) -> Result<[[f64; 4]; 4], String> {
    let text = std::fs::read_to_string(name).map_err(|_| format!("{name} not found."))?;
    let mut rows = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("");
        let vals: Result<Vec<f64>, _> = line.split_whitespace().map(str::parse::<f64>).collect();
        let vals = vals.map_err(|_| "a value loadtxt cannot read".to_owned())?;
        if !vals.is_empty() {
            rows.push(vals);
        }
    }
    if rows.len() != 4 || rows.iter().any(|r| r.len() != 4) {
        return Err("a matrix that is not 4 x 4".to_owned());
    }
    let mut a = [[0.0f64; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            a[i][j] = rows[i][j];
        }
    }
    Ok(a)
}

/// `aff2rigid <input2standard mat> <output mat>`.
pub fn aff2rigid(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let _ = (settings, threads);
    if args.is_empty() {
        let _ = writeln!(out, "Usage: aff2rigid <input2standard mat> <output mat>");
        let _ = writeln!(out, " ");
        let _ = writeln!(
            out,
            "       First argument is the FLIRT transform (12 DOF) from the input image to standard"
        );
        let _ = writeln!(
            out,
            "       Second argument is the output matrix which will go from the input image to standard space (6 DOF)"
        );
        let _ = writeln!(
            out,
            "          aligning the AC, the AC-PC line and the mid-sagittal plane (in order of decreasing accuracy)"
        );
        return 1;
    }
    let a = match loadtxt4(&args[0]) {
        Ok(a) => a,
        Err(e) => {
            return refuse(
                err,
                &format!("{e} (aff2rigid stops with Python's traceback, exit status 1)"),
            );
        }
    };
    // ainv = linalg.inv(a): LAPACK dgesv of A (Fortran order) against the identity.
    let mut lu = [0.0f64; 16];
    for i in 0..4 {
        for j in 0..4 {
            lu[j * 4 + i] = a[i][j];
        }
    }
    let mut b = [0.0f64; 16];
    for i in 0..4 {
        b[i * 4 + i] = 1.0;
    }
    let mut ipiv = [0i32; 4];
    if lapack::gesv(&mut lu, 4, &mut ipiv, &mut b, 4) != 0 {
        return refuse(
            err,
            "a singular matrix (numpy raises LinAlgError: Singular matrix, exit status 1)",
        );
    }
    let mut ainv = [[0.0f64; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            ainv[i][j] = b[j * 4 + i];
        }
    }
    let x1 = [91.0, 129.0, 67.0, 1.0];
    let x2 = [91.0, 100.0, 70.0, 1.0];
    let x3 = [91.0, 129.0, 117.0, 1.0];
    let sub4 = |p: &[f64; 4], q: &[f64; 4]| [p[0] - q[0], p[1] - q[1], p[2] - q[2], p[3] - q[3]];
    let div4 = |p: &[f64; 4], k: f64| [p[0] / k, p[1] / k, p[2] / k, p[3] / k];
    let ortho = |v21: &[f64; 4], v31: &[f64; 4]| {
        // v31 - multiply(v31.T * v21, v21)
        let d = dot4(v31, v21);
        [
            v31[0] - d * v21[0],
            v31[1] - d * v21[1],
            v31[2] - d * v21[2],
            v31[3] - d * v21[3],
        ]
    };
    let mut v21 = sub4(&x2, &x1);
    let mut v31 = sub4(&x3, &x1);
    v21 = div4(&v21, norm4(&v21));
    v31 = ortho(&v21, &v31);
    v31 = div4(&v31, norm4(&v31));
    let t = cross3(&v21[0..3], &v31[0..3]);
    // FSL-81: `v41=mat(zeros((4,1)))`: numpy 2 (FSL 6.0.7.23's 2.4.6) no longer has `mat`, so
    // aff2rigid stops here with a NameError and writes nothing. Corrected by default (rule A:
    // the matrix the script computes); FSL's traceback and exit status 1 with `reproduce`.
    let line = |e: &mut dyn Write| {
        let _ = writeln!(e, "{PREFIX}: {}", Notice::new(&defects::FSL_81, mode, if mode.fix() {
            "the matrix is computed (aff2rigid stops with NameError: name 'mat' is not defined)"
                .to_owned()
        } else {
            "aff2rigid stops with NameError: name 'mat' is not defined, as FSL 6.0.7.23's"
                .to_owned()
        }).line());
    };
    if mode.reproduce() {
        let _ = writeln!(err, "Traceback (most recent call last):");
        let _ = writeln!(err, "  File \"aff2rigid\", line 105, in <module>");
        let _ = writeln!(err, "    v41=mat(zeros((4,1)))");
        let _ = writeln!(err, "        ^^^");
        let _ = writeln!(err, "NameError: name 'mat' is not defined");
        line(err);
        return 1;
    }
    if args.len() < 2 {
        line(err);
        return refuse(
            err,
            "no output matrix (aff2rigid stops with Python's IndexError traceback, exit status 1)",
        );
    }
    let v41 = [t[0], t[1], t[2], 0.0];
    let mut w21 = gemv4(&ainv, &v21);
    let mut w31 = gemv4(&ainv, &v31);
    w21 = div4(&w21, norm4(&w21));
    w31 = ortho(&w21, &w31);
    w31 = div4(&w31, norm4(&w31));
    let t = cross3(&w21[0..3], &w31[0..3]);
    let w41 = [t[0], t[1], t[2], 0.0];
    let mut r1 = [[0.0f64; 4]; 4];
    let mut r2 = [[0.0f64; 4]; 4];
    for i in 0..4 {
        r1[i][i] = 1.0;
        r2[i][i] = 1.0;
    }
    for i in 0..4 {
        r1[i][0] = w21[i];
        r1[i][1] = w31[i];
        r1[i][2] = w41[i];
        r2[0][i] = v21[i];
        r2[1][i] = v31[i];
        r2[2][i] = v41[i];
    }
    let tr = |m: &[[f64; 4]; 4]| {
        let mut o = [[0.0f64; 4]; 4];
        for i in 0..4 {
            for j in 0..4 {
                o[i][j] = m[j][i];
            }
        }
        o
    };
    let mut r = gemm4(&tr(&r2), &tr(&r1));
    let acnat = gemv4(&ainv, &x1);
    let rac = gemv4(&r, &acnat);
    let trans = sub4(&x1, &rac);
    for i in 0..3 {
        r[i][3] = trans[i];
    }
    let mut text = String::new();
    for row in &r {
        let cols: Vec<String> = row.iter().map(|&v| py_f(v)).collect();
        text.push_str(&cols.join(" "));
        text.push('\n');
    }
    if let Err(e) = std::fs::write(&args[1], text) {
        line(err);
        return refuse(err, &format!("cannot write {}: {e}", args[1]));
    }
    line(err);
    0
}
