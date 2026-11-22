// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The cases of the fs-getfullpath probe (`validation/probes/fs-getfullpath/check.py`) and the
//! port's side of them: the scripts' calls and every way tcsh reads a name again (blanks,
//! patterns, braces, `~`, `=N`, `+N` folders, leading dashes, backslashes, odd folders).
//!
//! Every case runs in a folder holding the same files ([`fixture`]: on systems other than Unix
//! without the names they cannot hold). A case is one line: `gfp <flag> <folder to run in>
//! <n> <NAME=value>... <n> <argument>...`, each text in hexadecimal (`@` for an empty one), `<D>`
//! the case's folder; `flag` `-` for a plain case (no character tcsh reads again: the same on every
//! system and in both modes), `u` for one only Unix gives the same way. The line of a run: `EXIT
//! <status> | <standard output> | <standard error>`, the case's folder (as given and as its
//! physical path) as `<DIR>`; on systems other than Unix the standard output's `\` as `/`.

#![allow(dead_code)]

use std::path::Path;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_volumes::getfullpath::main::main;

fn hex(s: &str) -> String {
    if s.is_empty() {
        return "@".to_owned();
    }
    s.bytes().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> String {
    if s == "@" {
        return String::new();
    }
    let b: Vec<u8> = (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
        .collect();
    String::from_utf8(b).unwrap()
}

/// The folders and files every case's folder holds (folders end with `/`); the second list only
/// on Unix.
pub fn fixture() -> (Vec<&'static str>, Vec<&'static str>) {
    let portable = vec![
        "d1/",
        "d1/x.mgz",
        "d1/y.mgz",
        "d1/q r",
        "a b/",
        "a b/f",
        "a  b/",
        "real/",
        "sp ace/",
        "+1/",
        "+1/f",
        "+0/",
        "+/",
        "=1/",
        "sub/mri/",
        "sub/mri/aseg.mgz",
        "afile",
        "a",
        "B",
        "_c",
        ".h",
        "q r",
    ];
    let unix = vec![
        "d1/*star",
        "*s",
        "q*dir/",
        "nomatch[x]/",
        "tab\tdir/",
        "bs\\tx/",
        "noperm/",
        "lnk",
    ];
    (portable, unix)
}

/// A case before it is written: its flag, the folder it runs in, its variables, its arguments.
type Draft<'a> = (char, &'a str, Vec<(&'a str, &'a str)>, Vec<String>);

/// Every case of the probe.
pub fn all_cases() -> Vec<String> {
    let mut c: Vec<Draft> = Vec::new();
    let home = vec![("HOME", "/nonexistent-home")];
    let s = |v: &[&str]| v.iter().map(|x| (*x).to_owned()).collect::<Vec<_>>();
    // plain: the scripts' calls and names tcsh reads as they are
    for args in [
        s(&[]),
        s(&["<D>/sub/mri/aseg.mgz"]),
        s(&["sub/mri/aseg.mgz"]),
        s(&["d1/x.mgz"]),
        s(&["x"]),
        s(&["d1/"]),
        s(&["d1//x"]),
        s(&["a//"]),
        s(&["."]),
        s(&[".."]),
        s(&["d1/x.mgz", "extra"]),
        s(&["nodir/f"]),
        s(&["<D>/nodir/f"]),
        s(&["afile/f"]),
        s(&["a/."]),
        s(&["afile"]),
        s(&["-"]),
        s(&["d1/-n"]),
        s(&["+/f"]),
        s(&["+x/f"]),
        s(&["x~"]),
        s(&["!x"]),
        s(&["x#y"]),
        s(&["x$"]),
        s(&["(x)"]),
        s(&["x'y"]),
        s(&["x\"y"]),
        s(&["x;"]),
        s(&["x>y"]),
        s(&["x|y"]),
        s(&["x&y"]),
        s(&["=/f"]),
        s(&["x}"]),
        s(&["x["]),
        s(&["x[a"]),
        s(&[".h"]),
        s(&["d1/x.mgz]"]),
        s(&["$HOME/f"]),
    ] {
        c.push(('-', "", home.clone(), args));
    }
    c.push(('-', "sub/mri", home.clone(), s(&["aseg.mgz"])));
    c.push(('-', "sub/mri", home.clone(), s(&["../mri/aseg.mgz"])));
    // tcsh reads them again
    for args in [
        s(&["d1/*.mgz"]),
        s(&["{}"]),
        s(&["a{}b"]),
        s(&["d1/z*.mgz"]),
        s(&["d1/{x,y}.mgz"]),
        s(&["d1/[x]*.mgz"]),
        s(&["d1/{x}.mgz"]),
        s(&["d1/{x,zz}.mgz"]),
        s(&["{a,b}"]),
        s(&["{,}"]),
        s(&["x{"]),
        s(&["a{b,c"]),
        s(&["x{a}"]),
        s(&["a b/f"]),
        s(&["a  b/f"]),
        s(&["a b"]),
        s(&["x yy"]),
        s(&["x y z"]),
        s(&["x 1"]),
        s(&["x =y"]),
        s(&["x y=z"]),
        s(&["x _y"]),
        s(&["x y-z"]),
        s(&["x y = z"]),
        s(&["x y ="]),
        s(&["x Y"]),
        s(&["x é"]),
        s(&["x\ty"]),
        s(&["d1/x.mgz d1/y.mgz"]),
        s(&["x fname=d1/y.mgz"]),
        s(&["x fname = d1/q"]),
        s(&["x y=zz*"]),
        s(&["d1/*.mgz ab"]),
        s(&["~/f"]),
        s(&["~"]),
        s(&["~nosuchuser/f"]),
        s(&["=1/f"]),
        s(&["=0/f"]),
        s(&["=-/f"]),
        s(&["+1/f"]),
        s(&["-n"]),
        s(&["-e"]),
        s(&["-/f"]),
        s(&["--"]),
        s(&["--x"]),
        s(&["-nq"]),
        s(&[""]),
        s(&["   "]),
        s(&["a\\tb"]),
        s(&["x\\ny"]),
        s(&["x\\cy"]),
        s(&["x\\\\y"]),
        s(&["x\\0101y"]),
        s(&["x\\ey"]),
        s(&["x\\ay"]),
        s(&["a\\101b"]),
        s(&["a\\"]),
        s(&["noperm/f"]),
        s(&["lnk/f"]),
        s(&["/"]),
        s(&["//"]),
        s(&["/."]),
        s(&["///x"]),
        s(&["d1/[*]star"]),
        s(&["q?r"]),
        s(&["q*"]),
        s(&[".*"]),
        s(&["*"]),
        s(&["[[:alpha:]]"]),
        s(&["[a-]"]),
        s(&["[]x]"]),
        s(&["d1/[^x]*.mgz"]),
        s(&["d1/[!x]*.mgz"]),
        s(&["d1/[a-y]*.mgz"]),
        s(&["d*/x.mgz"]),
        s(&["*/f"]),
        s(&["nomatch[x]/f"]),
        s(&["tab\tdir/f"]),
        s(&["bs\\tx/f"]),
    ] {
        c.push(('u', "", home.clone(), args));
    }
    c.push(('u', "", vec![], s(&["~/f"])));
    c.push(('u', "sp ace", home.clone(), s(&["x"])));
    c.push(('u', "a  b", home.clone(), s(&["f"])));
    c.push(('u', "q*dir", home.clone(), s(&["x"])));
    c.push(('u', "nomatch[x]", home.clone(), s(&["x"])));
    c.push(('u', "tab\tdir", home.clone(), s(&["x"])));
    c.push(('u', "bs\\tx", home.clone(), s(&["x"])));
    c.push(('u', "+0", home.clone(), s(&["../+0/x"])));
    // forms the model does not reproduce (counted apart)
    for args in [
        s(&["x\ny"]),
        s(&["x y[1]=z"]),
        s(&["--help"]),
        s(&["--version"]),
        s(&["-z"]),
        s(&["--zero"]),
        s(&["+0/f"]),
    ] {
        c.push(('u', "", home.clone(), args));
    }
    c.into_iter()
        .map(|(flag, cwd, env, args)| {
            let mut t = vec!["gfp".to_owned(), flag.to_string(), hex(cwd)];
            t.push(env.len().to_string());
            t.extend(env.iter().map(|(k, v)| hex(&format!("{k}={v}"))));
            t.push(args.len().to_string());
            t.extend(args.iter().map(|a| hex(a)));
            t.join(" ")
        })
        .collect()
}

/// Seeded names made of the pieces tcsh reads again (`n` cases of seed `seed`), each run in the
/// case's folder or one of its folders.
pub fn random_cases(seed: u64, n: usize) -> Vec<String> {
    const PIECES: [&str; 40] = [
        "d1", "x.mgz", "y", "a b", "q r", "*", "?", "[x]", "[^x]", "[a-c]", "{a,b}", "{x,y}", "{}",
        "~", "=1", "=0", "\\t", "\\c", "\\0101", "/", "/", "/", ".", "..", "-", "+1", "+", "mgz",
        "_c", "B", "real", "sub", "mri", "aseg.mgz", "afile", " ", "  ", "y=z", "#", "$HOME",
    ];
    const FOLDERS: [&str; 6] = ["", "", "d1", "sub/mri", "sp ace", "a  b"];
    let mut state = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    let mut next = |m: usize| {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((state >> 33) as usize) % m
    };
    (0..n)
        .map(|_| {
            let k = 1 + next(4);
            let name: String = (0..k).map(|_| PIECES[next(PIECES.len())]).collect();
            let cwd = FOLDERS[next(FOLDERS.len())];
            let mut t = vec!["gfp".to_owned(), "u".to_owned(), hex(cwd), "1".to_owned()];
            t.push(hex("HOME=/nonexistent-home"));
            t.push("1".to_owned());
            t.push(hex(&name));
            t.join(" ")
        })
        .collect()
}

/// The golden cases (`tests/getfullpath_golden.rs`): seg2cc's call (an absolute name), a relative
/// name, a name in the folder itself, a folder that does not exist, no argument.
pub fn golden_cases() -> Vec<String> {
    let keep = [1usize, 3, 4, 11, 0];
    let all = all_cases();
    keep.iter().map(|&k| all[k].clone()).collect()
}

/// Whether a case is plain (the same on every system and in both modes).
pub fn plain(line: &str) -> bool {
    line.split(' ').nth(1) == Some("-")
}

#[cfg(unix)]
fn set_mode(p: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode)).unwrap();
}

/// The fixture made in `dir`.
pub fn make_fixture(dir: &Path) {
    let (portable, unix) = fixture();
    let all: Vec<&str> = if cfg!(unix) {
        portable.iter().chain(unix.iter()).copied().collect()
    } else {
        portable
    };
    for name in all {
        if name == "lnk" {
            #[cfg(unix)]
            std::os::unix::fs::symlink("real", dir.join("lnk")).unwrap();
            continue;
        }
        if let Some(d) = name.strip_suffix('/') {
            std::fs::create_dir_all(dir.join(d)).unwrap();
        } else {
            std::fs::write(dir.join(name), b"").unwrap();
        }
    }
    #[cfg(unix)]
    set_mode(&dir.join("noperm"), 0o000);
}

/// The fixture's read-only folder made removable again.
pub fn unmake_fixture(dir: &Path) {
    let _ = dir; // used on Unix only
    #[cfg(unix)]
    if dir.join("noperm").exists() {
        set_mode(&dir.join("noperm"), 0o755);
    }
}

fn escape(b: &[u8]) -> String {
    b.iter()
        .map(|&c| {
            if (0x20..=0x7e).contains(&c) && c != b'\\' {
                (c as char).to_string()
            } else {
                format!("\\x{c:02x}")
            }
        })
        .collect()
}

/// The port's line of `line` in the folder `dir` (holding the fixture), and the defects it noted
/// (with `UNMODELLED` when the model does not reproduce tcsh's handling).
pub fn run_line_in(line: &str, mode: UpstreamDefects, dir: &Path) -> (String, Vec<&'static str>) {
    let t: Vec<&str> = line.split(' ').collect();
    assert_eq!(t[0], "gfp");
    let d = dir.to_string_lossy().into_owned();
    let place = |s: &str| unhex(s).replace("<D>", &d);
    let cwd = dir.join(unhex(t[2]));
    let nenv: usize = t[3].parse().unwrap();
    let env: Vec<(String, String)> = t[4..4 + nenv]
        .iter()
        .map(|s| {
            let kv = place(s);
            let (k, v) = kv.split_once('=').unwrap();
            (k.to_owned(), v.to_owned())
        })
        .collect();
    let i = 4 + nenv;
    let nargs: usize = t[i].parse().unwrap();
    let args: Vec<String> = t[i + 1..i + 1 + nargs].iter().map(|s| place(s)).collect();
    let mut g = Globals::new(mode);
    let o = main(&mut g, &Environment::from_pairs(env), &cwd, &args);
    let mut out = o.stdout.clone();
    if !cfg!(unix) {
        for b in &mut out {
            if *b == b'\\' {
                *b = b'/';
            }
        }
    }
    let mut masks = vec![escape(d.as_bytes())];
    if let Some(p) = larmorx_freesurfer_volumes::csh::physical(dir) {
        let p = if cfg!(unix) { p } else { p.replace('\\', "/") };
        masks.insert(0, escape(p.as_bytes()));
    }
    let mask = |b: &[u8]| {
        let mut s = escape(b);
        for m in &masks {
            s = s.replace(m, "<DIR>");
        }
        s
    };
    let text = format!("EXIT {} | {} | {}", o.status, mask(&out), mask(&o.stderr));
    let mut ids: Vec<&'static str> = g.notices.as_slice().iter().map(|n| n.defect.id).collect();
    if g.notices
        .as_slice()
        .iter()
        .any(|n| n.detail.contains("is not reproduced"))
    {
        ids.push("UNMODELLED");
    }
    (text, ids)
}

/// [`run_line_in`] in a fresh temporary folder holding the fixture.
pub fn run_line(line: &str, mode: UpstreamDefects) -> (String, Vec<&'static str>) {
    let dir = tempfile::tempdir().expect("a temporary folder");
    make_fixture(dir.path());
    let r = run_line_in(line, mode, dir.path());
    unmake_fixture(dir.path());
    r
}
