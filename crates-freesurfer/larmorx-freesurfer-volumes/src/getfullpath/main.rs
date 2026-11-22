// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `getfullpath`'s script (`scripts/getfullpath` 21-42): the program's entry point [`run`]
//! (registered as `getfullpath` in the crate's `PROGRAMS`) and [`main`].
//!
//! `getfullpath <name> [more ignored]`. No argument: `getfullpath filename` on standard output,
//! status 1. The name's folder (GNU `dirname`) must exist (`ERROR: cannot find <folder>`, status 1)
//! and be enterable (`<folder>: Not a directory.` or `<folder>: Permission denied.` on standard
//! error, status 1); then its physical path (links resolved, `/bin/pwd`), `/`, the name's last
//! component (GNU `basename`), a new line; status 0.
//!
//! The script never quotes its variables, so tcsh reads the name, the folder and the path again
//! as shell words ([`crate::csh`]); `reproduce` gives what it does, the default the name as given:
//! - **FS-1173**: blanks split the name (the first word used, the next ones read as variable
//!   assignments: `set: Variable name must begin with a letter.`, `... must contain alphanumeric
//!   characters.`), patterns (`*`, `?`, `[...]`, `{...}`) are matched against the files (`set: No
//!   match.`, `if: Expression Syntax.` for several), a leading `~` or `=N` is a home folder or the
//!   directory stack, a folder `+N` is taken as the stack (`pushd: Directory stack not that
//!   deep.`), an empty name gives `dirname`'s and `if`'s errors.
//! - **FS-1174**: a backslash sequence in the printed path is an escape of tcsh's `echo` (`\t` a
//!   tab, `\c` the end of the output, `\0nnn` a byte).
//! - **FS-1175**: a name starting with `-` (but `-` alone) is taken by `dirname` as its options
//!   (`dirname: invalid option -- 'x'`, `missing operand`, then `if: Missing file name.`).
//! - **FS-1176**: a folder whose physical path holds blanks or pattern characters: its blanks
//!   collapse to one space each, its patterns are matched (`echo: No match.`).
//!
//! Each with a notice when it changed the result. Forms of tcsh this model does not reproduce (a
//! new line in the name, subscripted assignments, `dirname`'s own `--help`, `--version`, `-z`)
//! give the default's result with FS-1173's notice saying so, in both modes.

use std::io::Write;
use std::path::{Path, PathBuf};

use larmorx_core::defects::{Defect, FS_1173, FS_1174, FS_1175, FS_1176};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::run::{Environment, Settings};

use crate::csh::{self, Fail, Shell};

/// The program's name as `argv[0]` gives it (FreeSurfer's scripts run it by name).
pub const NAME: &str = "getfullpath";

/// The variables tcsh 6.21 sets at its start (`tcsh -f`, from the environment): a subscripted
/// assignment to one of them is not reproduced.
const TCSH_VARIABLES: [&str; 26] = [
    "addsuffix", "anyerror", "autoexpand", "csubstnonl", "cwd", "dirstack", "echo_style", "edit",
    "gid", "group", "history", "home", "killring", "loginsh", "owd", "path", "prompt", "prompt2",
    "prompt3", "promptchars", "shell", "shlvl", "status", "tcsh", "term", "user",
];

/// What a run gives: its exit status and the bytes of its two streams.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    pub status: u8,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl Outcome {
    fn out(text: &[u8], status: u8) -> Outcome {
        Outcome {
            status,
            stdout: text.to_vec(),
            stderr: Vec::new(),
        }
    }
}

/// The model's result: an outcome, or a form of tcsh it does not reproduce.
enum Model {
    Done(Outcome),
    Unmodelled(&'static str),
}

/// `getfullpath`'s entry point: its original arguments (without the program's name), the
/// settings, the two streams; the exit status. The notices of the run follow on standard error.
pub fn run(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mut g = Globals::new(settings.mode);
    let cwd = match &settings.dir {
        Some(d) => d.clone(),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    };
    let o = main(&mut g, &settings.env, &cwd, args);
    let _ = out.write_all(&o.stdout);
    let _ = out.flush();
    let _ = err.write_all(&o.stderr);
    g.notices.print(err);
    o.status
}

/// The script on `args` in the folder `cwd` (absolute): the default's outcome or, with
/// `reproduce`, tcsh's; a notice when they differ.
pub fn main(g: &mut Globals, env: &Environment, cwd: &Path, args: &[String]) -> Outcome {
    // the folder as tcsh's `$cwd` holds it (no final `/`)
    let cwd: PathBuf = cwd.components().collect();
    let cwd = cwd.as_path();
    let literal = as_named(cwd, args);
    let tcsh = match script(env, cwd, args) {
        Model::Done(o) => o,
        Model::Unmodelled(what) => {
            g.notices.add(
                &FS_1173,
                format!(
                    "getfullpath: tcsh's handling of {what} is not reproduced: the name as given, in both modes"
                ),
            );
            return literal;
        }
    };
    if tcsh == literal {
        return literal;
    }
    let (defect, what) = cause(args, &literal);
    g.notices.add(
        defect,
        if g.notices.reproduce() {
            format!("getfullpath: {what}: tcsh's result, as in FreeSurfer")
        } else {
            format!("getfullpath: {what}: the name as given (FreeSurfer's tcsh reads it again)")
        },
    );
    if g.notices.reproduce() { tcsh } else { literal }
}

/// Which defect made tcsh's result differ.
fn cause(args: &[String], literal: &Outcome) -> (&'static Defect, &'static str) {
    let name = &args[0];
    let special = |s: &str| {
        s.contains([' ', '\t', '*', '?', '{']) || csh::has_pattern(s) || s.is_empty()
    };
    let folder = csh::dirname(name);
    let stack = folder.starts_with('+') && folder[1..].chars().all(|c| c.is_ascii_digit());
    if name.starts_with('-') && name != "-" {
        (&FS_1175, "a name starting with '-' is taken as dirname's options")
    } else if special(name) || name.starts_with(['~', '=']) || stack {
        (
            &FS_1173,
            "the name holds blanks, patterns or shell expansions",
        )
    } else if name.contains('\\') || literal.stdout.contains(&b'\\') {
        (&FS_1174, "a backslash in the printed path")
    } else {
        (&FS_1176, "the folder's path holds blanks or patterns")
    }
}

/// The default: the name as given.
fn as_named(cwd: &Path, args: &[String]) -> Outcome {
    let Some(name) = args.first().filter(|n| !n.is_empty()) else {
        return Outcome::out(b"getfullpath filename\n", 1);
    };
    let folder = csh::dirname(name);
    let at = if folder.starts_with('/') {
        PathBuf::from(&folder)
    } else {
        cwd.join(&folder)
    };
    if std::fs::metadata(&at).is_err() {
        return Outcome::out(format!("ERROR: cannot find {folder}\n").as_bytes(), 1);
    }
    match csh::enter(cwd, &folder) {
        Err(msg) => Outcome {
            status: 1,
            stdout: Vec::new(),
            stderr: format!("{msg}\n").into_bytes(),
        },
        Ok(phys) => Outcome::out(
            format!("{}/{}\n", phys.to_string_lossy(), csh::basename(name)).as_bytes(),
            0,
        ),
    }
}

/// A failure of the script after `stderr` so far.
fn failed(stdout: Vec<u8>, mut stderr: String, f: Fail) -> Model {
    stderr.push_str(&f.stderr);
    Model::Done(Outcome {
        status: 1,
        stdout,
        stderr: stderr.into_bytes(),
    })
}

/// The words `set <var> = `...`` gives a command substitution's output: split, each expanded
/// (matched as a pattern only when `glob`: the command's text held `*`, `?` or `[`), `set: No
/// match.` when patterns matched nothing.
fn set_output(sh: &Shell, output: &str, glob: bool) -> Result<Vec<String>, Fail> {
    let words = csh::split_output(output);
    let mut out = Vec::new();
    let (mut patterns, mut matched) = (false, false);
    for w in words {
        let (ws, st) = sh.expand_piece_with(&w, glob)?;
        if let Some(m) = st {
            patterns = true;
            matched |= m;
        }
        out.extend(ws);
    }
    if patterns && !matched {
        return Err(Fail::new("set: No match."));
    }
    Ok(out)
}

/// A coreutils program (`dirname`, `basename`) on its arguments in a command substitution: its
/// output, its messages added to `stderr`; `None` for a form not reproduced.
fn coreutil(prog: &str, args: &[String], stderr: &mut String) -> Option<String> {
    match csh::coreutils_options(prog, args) {
        Err(text) => {
            stderr.push_str(&text);
            Some(String::new())
        }
        // the help or version text: many words (`if: Expression Syntax.` follows)
        Ok(Some(csh::HELP | csh::VERSION)) => Some(format!("{prog} text\n")),
        Ok(Some(_)) => None,
        Ok(None) => {
            let mut ops = Vec::new();
            let mut end = false;
            for a in args {
                if a == "--" && !end {
                    end = true;
                    continue;
                }
                ops.push(a.as_str());
            }
            if prog == "dirname" {
                Some(ops.iter().map(|o| csh::dirname(o) + "\n").collect())
            } else if ops.len() == 1 {
                Some(csh::basename(ops[0]) + "\n")
            } else if ops.len() == 2 {
                let b = csh::basename(ops[0]);
                let s = ops[1];
                Some(if b != s && b.ends_with(s) {
                    b[..b.len() - s.len()].to_owned() + "\n"
                } else {
                    b + "\n"
                })
            } else {
                None
            }
        }
    }
}

/// tcsh running the script.
fn script(env: &Environment, cwd: &Path, args: &[String]) -> Model {
    let mut stderr = String::new();
    let sh = Shell {
        env,
        cwd: cwd.to_path_buf(),
        stack: Vec::new(),
    };
    // if($#argv < 1): the usage
    let Some(arg) = args.first() else {
        return Model::Done(Outcome::out(b"getfullpath filename\n", 1));
    };
    if arg.contains(['\n', '\r']) {
        return Model::Unmodelled("a new line in the name");
    }
    if csh::has_range(arg) {
        return Model::Unmodelled("a pattern's range ([a-c])");
    }
    // set fname = $argv[1];
    let pieces = csh::split_blanks(arg);
    let mut fname = match pieces.first() {
        None => vec![String::new()],
        Some(v) => match sh.expand_piece(v) {
            Err(f) => return failed(Vec::new(), stderr, f),
            Ok((_, Some(false))) => return failed(Vec::new(), stderr, Fail::new("set: No match.")),
            Ok((ws, _)) => ws,
        },
    };
    let mut i = 1;
    while i < pieces.len() {
        let p = &pieces[i];
        let (name, inline) = match p.find('=') {
            Some(k) => (&p[..k], Some(&p[k + 1..])),
            None => (p.as_str(), None),
        };
        let first = name.chars().next();
        if !first.is_some_and(|c| c.is_ascii_alphabetic() || c == '_') {
            return failed(
                Vec::new(),
                stderr,
                Fail::new("set: Variable name must begin with a letter."),
            );
        }
        if let Some(k) = name.find('[') {
            // `name[i]=value`: `Subscript error.` for text after the `]`, an undefined variable's
            // message, a shell variable's assignment not reproduced
            let base = &name[..k];
            let closed = name.ends_with(']') && name[k + 1..name.len() - 1].find(']').is_none();
            if !base.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return failed(
                    Vec::new(),
                    stderr,
                    Fail::new("set: Variable name must contain alphanumeric characters."),
                );
            }
            // the subscript a number, a range of numbers or `*` (`r[^x]`: an error)
            let sub = if closed { &name[k + 1..name.len() - 1] } else { "" };
            let number = |t: &str| t.chars().all(|c| c.is_ascii_digit());
            let valid = sub == "*"
                || (!sub.is_empty()
                    && match sub.split_once('-') {
                        Some((a, b)) => number(a) && number(b) && !(a.is_empty() && b.is_empty()),
                        None => number(sub),
                    });
            if !valid {
                return failed(Vec::new(), stderr, Fail::new("set: Subscript error."));
            }
            if TCSH_VARIABLES.contains(&base) || base == "fname" || base == "argv" {
                return Model::Unmodelled("a subscripted assignment to a shell variable");
            }
            return failed(
                Vec::new(),
                stderr,
                Fail::new(&format!("{base}: Undefined variable.")),
            );
        }
        if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return failed(
                Vec::new(),
                stderr,
                Fail::new("set: Variable name must contain alphanumeric characters."),
            );
        }
        let value = if let Some(v) = inline {
            i += 1;
            Some(v.to_owned())
        } else if pieces.get(i + 1).is_some_and(|x| x == "=") {
            let v = pieces.get(i + 2).cloned();
            i += 3;
            Some(v.unwrap_or_default())
        } else {
            i += 1;
            None
        };
        let words = match value {
            Some(v) if !v.is_empty() => match sh.expand_piece(&v) {
                Err(f) => return failed(Vec::new(), stderr, f),
                Ok((_, Some(false))) => {
                    return failed(Vec::new(), stderr, Fail::new("set: No match."));
                }
                Ok((ws, _)) => ws,
            },
            _ => vec![String::new()],
        };
        if name == "fname" {
            fname = words;
        }
    }
    // set dirname = `dirname $fname`;
    // the command's text holds fname's words: their pattern characters make its output matched
    let glob = fname.iter().any(|w| w.contains(['*', '?', '[']));
    let dirname_out = match sh.args("dirname", &fname) {
        Err(f) => {
            stderr.push_str(&f.stderr);
            String::new()
        }
        Ok(a) => match coreutil("dirname", &a, &mut stderr) {
            Some(o) => o,
            None => return Model::Unmodelled("dirname's own options"),
        },
    };
    let dirname = match set_output(&sh, &dirname_out, glob) {
        Err(f) => return failed(Vec::new(), stderr, f),
        Ok(w) => w,
    };
    // if(! -e $dirname): the words split, not matched; `-e`'s operand matched alone
    let test: Vec<String> = dirname.iter().flat_map(|w| csh::split_blanks(w)).collect();
    // more words: `! -e w0` then arithmetic on them (`+`, `-`, `*`, `/`, `%`, each with the next
    // word as its operand, or 0 without one)
    let mut first_op: Option<&str> = None;
    match test.as_slice() {
        [] => return failed(Vec::new(), stderr, Fail::new("if: Missing file name.")),
        [_] => {}
        [_, rest @ ..] => {
            let is_op = |w: &str| matches!(w, "+" | "-" | "*" | "/" | "%");
            let mut k = 0;
            while k < rest.len() {
                let op = rest[k].as_str();
                if !is_op(op) {
                    return failed(Vec::new(), stderr, Fail::new("if: Expression Syntax."));
                }
                first_op.get_or_insert(op);
                k += 1;
                if k < rest.len() && !is_op(&rest[k]) {
                    if !rest[k].chars().all(|c| c.is_ascii_digit()) || rest[k].is_empty() {
                        return failed(Vec::new(), stderr, Fail::new("if: Expression Syntax."));
                    }
                    return Model::Unmodelled("an expression with a number");
                }
                if op == "/" || op == "%" {
                    let what = if op == "/" { "Division by 0." } else { "Mod by 0." };
                    return failed(Vec::new(), stderr, Fail::new(what));
                }
            }
        }
    }
    let operand = match sh.expand_piece(&test[0]) {
        Err(f) => return failed(Vec::new(), stderr, f),
        Ok((_, Some(false))) => {
            return failed(
                Vec::new(),
                stderr,
                Fail::new(&format!("{}: No match.", test[0])),
            );
        }
        Ok((w, _)) if w.len() > 1 => return Model::Unmodelled("an ambiguous file test"),
        Ok((w, _)) => w.into_iter().next().unwrap_or_default(),
    };
    let at = if operand.starts_with('/') {
        PathBuf::from(&operand)
    } else {
        cwd.join(&operand)
    };
    // the condition: `! -e w0`, times 0 when the first operator is `*`
    if std::fs::metadata(&at).is_err() && first_op != Some("*") {
        // echo "ERROR: cannot find $dirname"
        let line = format!("ERROR: cannot find {}", dirname.join(" "));
        return Model::Done(Outcome {
            status: 1,
            stdout: csh::echo(&[line]),
            stderr: stderr.into_bytes(),
        });
    }
    // pushd $dirname > /dev/null
    let target = match sh.args("pushd", &dirname) {
        Err(f) => return failed(Vec::new(), stderr, f),
        Ok(w) => w,
    };
    let new_cwd = match target.as_slice() {
        [] => return failed(Vec::new(), stderr, Fail::new("pushd: No other directory.")),
        // `+N` with N above 0 is the directory stack's N-th folder (`+0` a folder's name)
        [w] if w.len() > 1
            && w.starts_with('+')
            && w[1..].chars().all(|c| c.is_ascii_digit())
            && !w[1..].chars().all(|c| c == '0') =>
        {
            return failed(
                Vec::new(),
                stderr,
                Fail::new("pushd: Directory stack not that deep."),
            );
        }
        [w] => match csh::enter(cwd, w) {
            Err(msg) => return failed(Vec::new(), stderr, Fail::new(&msg)),
            Ok(p) => p,
        },
        _ => return failed(Vec::new(), stderr, Fail::new("pushd: Too many arguments.")),
    };
    let sh2 = Shell {
        env,
        cwd: new_cwd.clone(),
        stack: vec![cwd.to_path_buf()],
    };
    // set dirname = `pwd`;
    let dirname = match set_output(&sh2, &format!("{}\n", new_cwd.to_string_lossy()), false) {
        Err(f) => return failed(Vec::new(), stderr, f),
        Ok(w) => w,
    };
    // set base = `basename $fname`;
    let base_out = match sh2.args("basename", &fname) {
        Err(f) => {
            stderr.push_str(&f.stderr);
            String::new()
        }
        Ok(a) => match coreutil("basename", &a, &mut stderr) {
            Some(o) => o,
            None => return Model::Unmodelled("basename's own options or extra operands"),
        },
    };
    let base = match set_output(&sh2, &base_out, glob) {
        Err(f) => return failed(Vec::new(), stderr, f),
        Ok(w) => w,
    };
    // echo $dirname/$base
    let mut words: Vec<String> = dirname.clone();
    let joined = format!(
        "{}/{}",
        words.pop().unwrap_or_default(),
        base.first().cloned().unwrap_or_default()
    );
    words.push(joined);
    words.extend(base.iter().skip(1).cloned());
    let printed = match sh2.args("echo", &words) {
        Err(f) => return failed(Vec::new(), stderr, f),
        Ok(w) => w,
    };
    Model::Done(Outcome {
        status: 0,
        stdout: csh::echo(&printed),
        stderr: stderr.into_bytes(),
    })
}
