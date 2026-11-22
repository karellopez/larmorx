// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The cases of the fs-temp-file probe (`validation/probes/fs-temp-file/check.py`) and the port's
//! side of them: every option of `fs_temp_file`, the five variables of the temporary folder in
//! their order, folders that do not exist, cannot be written or are files, suffixes that make no
//! file, the parser's messages, the help and version options.
//!
//! A case is one line: `tmpf <flags> <n> <folder>... <n> <file>... <n> <read-only folder>...
//! <n> <link> <target>... <n> <NAME=value>... <n> <argument>...`, each text in hexadecimal (`@`
//! for an empty one); `<D>` in a value, a target or an argument is the case's folder. `flags`:
//! `u` for a case only Unix gives the same way (`/tmp`, folders made read-only, links), else `-`.
//! The line of a run: `EXIT <status> | <standard output> | <standard error> | <files>`: the case's
//! folder as `<DIR>`, each `tmp.` with six letters or digits after it as `tmp.XXXXXX`, the
//! `--all-info` fields that name the time, the user and the machine as `*`; the files every entry
//! under the case's folder (`D <name> <mode>`, `F <name> <mode> <size>`, `L <name>`; modes in
//! octal on Unix, `-` elsewhere) and, for a file the program made outside it (`/tmp`), `X <its
//! name> <mode> <size>` (it is then removed).

#![allow(dead_code)]

use std::io::Write as _;
use std::path::Path;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::TrackedErr;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_volumes::fs_temp_file::main::main;

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

/// One case: what the folder holds before the run, the variables and the arguments.
#[derive(Default)]
struct Case {
    unix: bool,
    dirs: Vec<&'static str>,
    files: Vec<&'static str>,
    read_only: Vec<&'static str>,
    links: Vec<(&'static str, &'static str)>,
    env: Vec<(&'static str, String)>,
    args: Vec<String>,
}

impl Case {
    fn line(&self) -> String {
        let mut t = vec![
            "tmpf".to_owned(),
            if self.unix { "u" } else { "-" }.to_owned(),
        ];
        for list in [&self.dirs, &self.files, &self.read_only] {
            t.push(list.len().to_string());
            t.extend(list.iter().map(|s| hex(s)));
        }
        t.push(self.links.len().to_string());
        for (l, target) in &self.links {
            t.push(hex(l));
            t.push(hex(target));
        }
        t.push(self.env.len().to_string());
        t.extend(self.env.iter().map(|(k, v)| hex(&format!("{k}={v}"))));
        t.push(self.args.len().to_string());
        t.extend(self.args.iter().map(|a| hex(a)));
        t.join(" ")
    }
}

fn a(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| (*s).to_owned()).collect()
}

fn e(pairs: &[(&'static str, &str)]) -> Vec<(&'static str, String)> {
    pairs.iter().map(|(k, v)| (*k, (*v).to_owned())).collect()
}

/// Every case of the probe.
pub fn all_cases() -> Vec<String> {
    let t = "<D>/t";
    let tt = vec!["t"];
    let mut c: Vec<Case> = Vec::new();
    // the variables, in their order
    c.push(Case {
        dirs: tt.clone(),
        env: e(&[("TMPDIR", t)]),
        ..Case::default()
    });
    c.push(Case {
        dirs: vec!["t", "f"],
        env: e(&[("FS_TMPDIR", "<D>/f"), ("TMPDIR", t)]),
        ..Case::default()
    });
    c.push(Case {
        dirs: tt.clone(),
        env: e(&[("FS_TMPDIR", "<D>/missing"), ("TMPDIR", t)]),
        ..Case::default()
    });
    c.push(Case {
        dirs: tt.clone(),
        env: e(&[("FS_TMPDIR", ""), ("TMP", t)]),
        ..Case::default()
    });
    for name in ["TMP", "TEMP", "TEMPDIR"] {
        c.push(Case {
            dirs: tt.clone(),
            env: e(&[(name, t)]),
            ..Case::default()
        });
    }
    c.push(Case {
        dirs: vec!["t", "u", "v"],
        env: e(&[("TMP", "<D>/u"), ("TEMPDIR", "<D>/v"), ("TEMP", t)]),
        ..Case::default()
    });
    // the last variable set but not usable: an error; an earlier one: /tmp
    c.push(Case {
        env: e(&[("TEMPDIR", "<D>/missing")]),
        ..Case::default()
    });
    c.push(Case {
        dirs: tt.clone(),
        env: e(&[("TMPDIR", "<D>/missing"), ("TEMPDIR", "<D>/missing2")]),
        ..Case::default()
    });
    c.push(Case {
        unix: true,
        env: e(&[("TMPDIR", "<D>/missing")]),
        ..Case::default()
    });
    c.push(Case {
        unix: true,
        ..Case::default()
    });
    c.push(Case {
        unix: true,
        env: e(&[("TEMPDIR", "")]),
        ..Case::default()
    });
    // a file, a read-only folder, a link
    c.push(Case {
        dirs: tt.clone(),
        files: vec!["afile"],
        env: e(&[("FS_TMPDIR", "<D>/afile"), ("TMPDIR", t)]),
        ..Case::default()
    });
    c.push(Case {
        files: vec!["afile"],
        env: e(&[("TEMPDIR", "<D>/afile")]),
        ..Case::default()
    });
    c.push(Case {
        unix: true,
        dirs: vec!["t", "ro"],
        read_only: vec!["ro"],
        env: e(&[("FS_TMPDIR", "<D>/ro"), ("TMPDIR", t)]),
        ..Case::default()
    });
    c.push(Case {
        unix: true,
        dirs: vec!["ro"],
        read_only: vec!["ro"],
        env: e(&[("TEMPDIR", "<D>/ro")]),
        ..Case::default()
    });
    c.push(Case {
        unix: true,
        dirs: vec!["real"],
        links: vec![("lnk", "<D>/real")],
        env: e(&[("TMPDIR", "<D>/lnk")]),
        ..Case::default()
    });
    c.push(Case {
        dirs: tt.clone(),
        env: e(&[("TMPDIR", "<D>/t/")]),
        ..Case::default()
    });
    c.push(Case {
        dirs: vec!["t", "a b"],
        env: e(&[("TMPDIR", "<D>/a b")]),
        ..Case::default()
    });
    c.push(Case {
        dirs: vec!["rel"],
        env: e(&[("TMPDIR", "rel")]),
        ..Case::default()
    });
    // the suffix
    for args in [
        a(&["-s", ".mgz"]),
        a(&["--suffix", ".nii.gz"]),
        a(&["-s", ""]),
        a(&["-s", "XXXXXX"]),
        a(&["-s", ".a b"]),
        a(&["-s", "-x"]),
        a(&["-s", ".mgz", "-s", ".lta"]),
        a(&["--suffix", ".dat", "-s", ".txt"]),
        a(&["-s", "/x"]),
        a(&["-s", "sub/x"]),
        vec!["-s".to_owned(), format!(".{}", "s".repeat(300))],
    ] {
        c.push(Case {
            dirs: tt.clone(),
            env: e(&[("TMPDIR", t)]),
            args,
            ..Case::default()
        });
    }
    // the base
    for (dirs, env, args, unix) in [
        (vec!["b"], vec![], a(&["-b", "<D>/b"]), false),
        (vec!["b"], vec![], a(&["--base", "b"]), false),
        (vec!["b"], vec![], a(&["-b", "<D>/b/"]), false),
        (vec!["t"], e(&[("TMPDIR", t)]), a(&["-b", "<D>/missing"]), false),
        (vec![], vec![], a(&["-b", "<D>/missing"]), true),
        (vec!["t"], e(&[("TMPDIR", t)]), a(&["-b", ""]), false),
        (
            vec!["b", "f"],
            e(&[("FS_TMPDIR", "<D>/f")]),
            a(&["-b", "<D>/b"]),
            false,
        ),
        (
            vec!["b", "t"],
            e(&[("TMPDIR", t)]),
            a(&["-b", "<D>/b", "-s", ".mgz"]),
            false,
        ),
        (
            vec!["b", "c"],
            vec![],
            a(&["--base", "<D>/b", "--base", "<D>/c"]),
            false,
        ),
        (
            vec!["t"],
            e(&[("TEMPDIR", "<D>/missing")]),
            a(&["-b", "<D>/missing2"]),
            false,
        ),
    ] {
        c.push(Case {
            unix,
            dirs,
            env,
            args,
            ..Case::default()
        });
    }
    c.push(Case {
        unix: true,
        dirs: vec!["t", "ro"],
        read_only: vec!["ro"],
        env: e(&[("TMPDIR", t)]),
        args: a(&["-b", "<D>/ro"]),
        ..Case::default()
    });
    c.push(Case {
        dirs: vec!["t"],
        files: vec!["afile"],
        env: e(&[("TMPDIR", t)]),
        args: a(&["-b", "<D>/afile"]),
        ..Case::default()
    });
    // --scratch (the probe's machines have no /scratch)
    for (dirs, env, args) in [
        (vec!["t"], e(&[("TMPDIR", t)]), a(&["--scratch"])),
        (vec!["f"], e(&[("FS_TMPDIR", "<D>/f")]), a(&["--scratch"])),
        (
            vec!["t"],
            e(&[("FS_TMPDIR", ""), ("TMPDIR", t)]),
            a(&["--scratch"]),
        ),
        (vec!["b"], vec![], a(&["--scratch", "-b", "<D>/b"])),
        (vec!["b"], vec![], a(&["-b", "<D>/b", "--scratch"])),
    ] {
        c.push(Case {
            dirs,
            env,
            args,
            ..Case::default()
        });
    }
    // the parser
    for args in [
        a(&["-s"]),
        a(&["-b"]),
        a(&["--suffix"]),
        a(&["-s", "-b"]),
        a(&["-s", ".a", "extra"]),
        a(&["extra"]),
        a(&["--foo"]),
        a(&["-s=.mgz"]),
        a(&["-sb"]),
        a(&["--scratch", "extra"]),
        a(&["--scratch", "--scratch"]),
        a(&["-x", "-s", ".a"]),
        a(&["--foo", "--help"]),
        a(&["-h"]),
        a(&["--help"]),
        a(&["-s", ".x", "--help"]),
        a(&["--version"]),
        a(&["--version", "-s", ".x"]),
        a(&["--help", "--version"]),
        a(&["--all-info"]),
        a(&["--all-info", "--version"]),
        a(&["--version", "--version"]),
        a(&["-version"]),
        a(&[""]),
    ] {
        c.push(Case {
            dirs: tt.clone(),
            env: e(&[("TMPDIR", t)]),
            args,
            ..Case::default()
        });
    }
    c.iter().map(Case::line).collect()
}

/// The golden cases (`tests/fs_temp_file_golden.rs`): the scripts' calls (no option, a suffix), a
/// base, a variable that cannot be used, the help.
pub fn golden_cases() -> Vec<String> {
    let t = "<D>/t";
    [
        Case {
            dirs: vec!["t"],
            env: e(&[("TMPDIR", t)]),
            ..Case::default()
        },
        Case {
            dirs: vec!["t"],
            env: e(&[("TMPDIR", t)]),
            args: a(&["--suffix", ".txt"]),
            ..Case::default()
        },
        Case {
            dirs: vec!["b", "t"],
            env: e(&[("TMPDIR", t)]),
            args: a(&["-b", "<D>/b", "-s", ".mgz"]),
            ..Case::default()
        },
        Case {
            dirs: vec!["t"],
            env: e(&[("FS_TMPDIR", "<D>/missing"), ("TEMP", t)]),
            ..Case::default()
        },
        Case {
            dirs: vec!["t"],
            env: e(&[("TMPDIR", t)]),
            args: a(&["--help"]),
            ..Case::default()
        },
    ]
    .iter()
    .map(Case::line)
    .collect()
}

/// Whether the case is one only Unix gives the same way.
pub fn unix_only(line: &str) -> bool {
    line.split(' ').nth(1) == Some("u")
}

/// The case's folder made, the variables and arguments with `<D>` replaced.
fn setup(line: &str, dir: &Path) -> (Vec<String>, Vec<(String, String)>) {
    let t: Vec<&str> = line.split(' ').collect();
    assert_eq!(t[0], "tmpf");
    let d = dir.to_string_lossy().into_owned();
    let place = |s: &str| unhex(s).replace("<D>", &d);
    let mut i = 2;
    let list = |i: &mut usize| {
        let n: usize = t[*i].parse().unwrap();
        let v: Vec<String> = t[*i + 1..*i + 1 + n].iter().map(|s| unhex(s)).collect();
        *i += 1 + n;
        v
    };
    // modes set, so that the listing does not depend on the process's umask
    for name in list(&mut i) {
        std::fs::create_dir_all(dir.join(&name)).unwrap();
        set_mode(&dir.join(name), 0o755);
    }
    for name in list(&mut i) {
        std::fs::write(dir.join(&name), b"").unwrap();
        set_mode(&dir.join(name), 0o644);
    }
    let read_only = list(&mut i);
    let nlinks: usize = t[i].parse().unwrap();
    for k in 0..nlinks {
        let (l, target) = (unhex(t[i + 1 + 2 * k]), place(t[i + 2 + 2 * k]));
        #[cfg(unix)]
        std::os::unix::fs::symlink(target, dir.join(l)).unwrap();
        #[cfg(not(unix))]
        let _ = (l, target);
    }
    i += 1 + 2 * nlinks;
    let nenv: usize = t[i].parse().unwrap();
    let env = t[i + 1..i + 1 + nenv]
        .iter()
        .map(|s| {
            let kv = place(s);
            let (k, v) = kv.split_once('=').unwrap();
            (k.to_owned(), v.to_owned())
        })
        .collect();
    i += 1 + nenv;
    let nargs: usize = t[i].parse().unwrap();
    let args = t[i + 1..i + 1 + nargs].iter().map(|s| place(s)).collect();
    for name in read_only {
        set_mode(&dir.join(name), 0o555);
    }
    (args, env)
}

#[cfg(unix)]
fn set_mode(p: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode)).unwrap();
}

#[cfg(not(unix))]
fn set_mode(_p: &Path, _mode: u32) {}

#[cfg(unix)]
fn mode_of(m: &std::fs::Metadata) -> String {
    use std::os::unix::fs::PermissionsExt;
    format!("{:o}", m.permissions().mode() & 0o777)
}

#[cfg(not(unix))]
fn mode_of(_m: &std::fs::Metadata) -> String {
    "-".to_owned()
}

/// Whether this process can make a file in a read-only folder (an administrator's account): the
/// read-only cases then do not hold.
pub fn writes_read_only_folders() -> bool {
    if !cfg!(unix) {
        return true;
    }
    let d = tempfile::tempdir().expect("a temporary folder");
    let ro = d.path().join("ro");
    std::fs::create_dir(&ro).unwrap();
    set_mode(&ro, 0o555);
    let made = std::fs::write(ro.join("x"), b"").is_ok();
    set_mode(&ro, 0o755);
    made
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

/// Each `tmp.` followed by six letters or digits as `tmp.XXXXXX`.
fn mask_letters(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        if b[i..].starts_with(b"tmp.")
            && b.len() >= i + 10
            && b[i + 4..i + 10].iter().all(u8::is_ascii_alphanumeric)
        {
            out.push_str("tmp.XXXXXX");
            i += 10;
        } else {
            out.push(b[i] as char);
            i += 1;
        }
    }
    out
}

/// `--all-info`'s time, user and machine as `*`.
fn mask_info(s: &str) -> String {
    const KEYS: [&str; 4] = ["TimeStamp: ", "User: ", "Machine: ", "PlatformVersion: "];
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    'outer: while i < b.len() {
        for k in KEYS {
            if b[i..].starts_with(k.as_bytes()) {
                let mut j = i + k.len();
                if j < b.len() && !b[j].is_ascii_whitespace() {
                    while j < b.len() && !b[j].is_ascii_whitespace() {
                        j += 1;
                    }
                    out.push_str(k);
                    out.push('*');
                    i = j;
                    continue 'outer;
                }
            }
        }
        out.push(b[i] as char);
        i += 1;
    }
    out
}

fn listing(dir: &Path, prefix: &str, out: &mut Vec<String>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap())
        .collect();
    entries.sort_by_key(|e| e.file_name().to_string_lossy().into_owned().into_bytes());
    for e in entries {
        let name = format!("{prefix}{}", e.file_name().to_string_lossy());
        let m = std::fs::symlink_metadata(e.path()).unwrap();
        if m.file_type().is_symlink() {
            out.push(format!("L {name}"));
        } else if m.is_dir() {
            out.push(format!("D {name} {}", mode_of(&m)));
            listing(&e.path(), &format!("{name}/"), out);
        } else {
            out.push(format!("F {name} {} {}", mode_of(&m), m.len()));
        }
    }
}

/// The port's line of `line` in the folder `dir`, and the defects it noted.
pub fn run_line_in(line: &str, mode: UpstreamDefects, dir: &Path) -> (String, Vec<&'static str>) {
    let (args, env) = setup(line, dir);
    let environment = Environment::from_pairs(env);
    let mut g = Globals::new(mode);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let status = {
        let mut terr = TrackedErr::new(&g, &mut err);
        match main(&mut g, &environment, Some(dir), &mut out, &mut terr, &args) {
            Ok(()) => 0,
            Err(f) => f.status(),
        }
    };
    let _ = out.flush();
    let d = escape(dir.to_string_lossy().as_bytes());
    let mask = |b: &[u8]| mask_info(&mask_letters(&escape(b).replace(&d, "<DIR>")));
    let mut files = Vec::new();
    // read-only folders made writable again, so that the listing and the removal can go in
    let mut top: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    top.sort_by_key(|p| e_name(p).into_bytes());
    for p in top {
        if p.is_dir() && !std::fs::symlink_metadata(&p).unwrap().file_type().is_symlink() {
            let m = std::fs::metadata(&p).unwrap();
            if m.permissions().readonly() || mode_of(&m) == "555" {
                set_mode(&p, 0o755);
                files.push(format!("RO {}", e_name(&p)));
            }
        }
    }
    let mut listed = Vec::new();
    listing(dir, "", &mut listed);
    files.extend(listed);
    // a file made outside the folder (/tmp): the last line printed, an absolute name elsewhere
    let printed = String::from_utf8_lossy(&out).into_owned();
    let last = printed.lines().last().unwrap_or("").to_owned();
    if status == 0 && !last.starts_with(&*dir.to_string_lossy()) {
        let p = Path::new(&last);
        if p.is_absolute()
            && let Ok(m) = std::fs::symlink_metadata(p)
        {
            files.push(format!(
                "X {} {} {}",
                mask_letters(&escape(last.as_bytes())),
                mode_of(&m),
                m.len()
            ));
            let _ = std::fs::remove_file(p);
        }
    }
    let text = format!(
        "EXIT {status} | {} | {} | {}",
        mask(&out),
        mask(&err),
        mask_letters(&files.join(" "))
    );
    let ids = g.notices.as_slice().iter().map(|n| n.defect.id).collect();
    (text, ids)
}

fn e_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// [`run_line_in`] in a fresh temporary folder.
pub fn run_line(line: &str, mode: UpstreamDefects) -> (String, Vec<&'static str>) {
    let dir = tempfile::tempdir().expect("a temporary folder");
    run_line_in(line, mode, dir.path())
}
