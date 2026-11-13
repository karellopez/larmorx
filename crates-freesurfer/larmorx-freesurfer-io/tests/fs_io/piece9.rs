// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs.io probe's piece-9 cases (relay 18): FreeSurfer's `ArgumentParser` (fs.core's
//! `argparse`). `apa`: a configuration (`A1` `addArgument(name, ...)`, `A2` `addArgument(short,
//! name, ...)`, `H` `addHelp` with an empty or an XML text), a command line (its first word the
//! program's name) parsed, then queries (`E` `exists`, `R` `retrieve` of a type: `i f b s` and
//! their vectors `I F B S`). The configurations are the seven programs' own and random ones; the
//! command lines mix the configured names, unknown flags, numbers and words of every kind
//! (`stoi`'s and `stof`'s edges: signs, spaces, overflow, tiny numbers, hexadecimal floats,
//! `nan(...)` payloads), `--version` and `--help`.

use std::cell::Cell;
use std::io::Write;

use larmorx_freesurfer_core::argparse::{self, ArgType, ArgumentParser, PLUS, STAR, Value};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::run::Environment;

use super::{Gen, Out, Work, hex};

/// The piece-9 commands.
pub const KINDS: [&str; 1] = ["apa"];

// ---------------------------------------------------------------- generation

/// One configured argument: (short, name, nargs, type, required); "" for no short name.
type Conf = (&'static str, &'static str, i8, i64, bool);

const PROGRAMS: [&[Conf]; 7] = [
    // AntsDenoiseImageFs (with addHelp first)
    &[
        ("-i", "--input", 1, 2, true),
        ("-o", "--output", 1, 2, true),
        ("", "--rician", 0, 0, false),
    ],
    // AntsN4BiasFieldCorrectionFs
    &[
        ("-i", "--input", 1, 2, true),
        ("-o", "--output", 1, 2, true),
        ("-m", "--mask", 1, 2, false),
        ("-s", "--shrink", 1, 3, false),
        ("-t", "--iters", PLUS, 3, false),
        ("-d", "--dtype", 1, 2, false),
        ("-r", "--replace-zeros", 3, 4, false),
        ("-e", "--threads-nondetermistic", 1, 3, false),
        ("-c", "--rescale", PLUS, 2, false),
        ("", "--rescale-only", 0, 0, false),
    ],
    // fs_temp_dir
    &[
        ("-b", "--base", 1, 0, false),
        ("", "--scratch", 0, 0, false),
    ],
    // fs_temp_file
    &[
        ("-s", "--suffix", 1, 0, false),
        ("-b", "--base", 1, 0, false),
        ("", "--scratch", 0, 0, false),
    ],
    // mris_defects_pointset (addHelp last)
    &[
        ("-s", "--surf", 1, 2, true),
        ("-d", "--defects", 1, 2, true),
        ("-o", "--out", 1, 2, true),
        ("-l", "--label", 1, 2, false),
        ("-c", "--control", 0, 0, false),
    ],
    // mris_remesh
    &[
        ("-i", "--input", 1, 2, true),
        ("-o", "--output", 1, 2, true),
        ("", "--remesh", 0, 0, false),
        ("", "--nvert", 1, 3, false),
        ("", "--edge-len", 1, 4, false),
        ("", "--desired-face-area", 1, 4, false),
        ("", "--iters", 1, 3, false),
    ],
    // mri_brainvol_stats (no help)
    &[("-s", "--subject", 1, 2, true), ("", "--sd", 1, 0, false)],
];

const NAMES: [&str; 22] = [
    "-i",
    "--input",
    "-o",
    "--output",
    "--rician",
    "-t",
    "--iters",
    "-x",
    "--long-name",
    "-h",
    "--help",
    "in",
    "out",
    "files",
    "",
    "-",
    "--",
    "ab",
    "abc",
    "-ab",
    "-v",
    "--v-x",
];

fn word(g: &mut Gen) -> String {
    g.pick(&[
        "3",
        "-2",
        " 7",
        "+4",
        "12abc",
        "2147483648",
        "-2147483649",
        "99999999999999999999",
        "1.5",
        "-0",
        "1e40",
        "1e-40",
        "1e-45",
        "0x1p-149",
        "0x1.8p3",
        "0x",
        "nan",
        "nan(12)",
        "-nan(0x7)",
        "nan(abc)",
        "inf",
        "-Infinity",
        ".5",
        "5.",
        ".",
        "e5",
        "abc",
        "",
        "true",
        "YES",
        "no",
        "False",
        "0",
        "1",
        "x y",
    ])
    .into()
}

pub fn generate(g: &mut Gen, kind: &str, _portable: bool) -> Vec<String> {
    let mut t: Vec<String> = vec![kind.into()];
    let mut conf: Vec<String> = Vec::new();
    let mut keys: Vec<String> = Vec::new();
    let mut kinds: Vec<(String, char)> = Vec::new();
    // each configured argument: a name to give it by, its count of inputs, type, required
    let mut args: Vec<(String, i8, i64, bool)> = Vec::new();
    let tc = |nargs: i8, ty: i64| {
        let ty = if ty == 0 {
            if nargs == 0 { 1 } else { 2 }
        } else {
            ty
        };
        let base = ['?', 'b', 's', 'i', 'f'][ty.clamp(0, 4) as usize];
        if nargs == 0 || nargs == 1 {
            base
        } else {
            base.to_ascii_uppercase()
        }
    };
    let help = g.below(3);
    if help > 0 && g.chance(2) {
        conf.push(format!("H {help}"));
    }
    if g.chance(2) {
        let p = PROGRAMS[g.below(7) as usize];
        for &(s, n, nargs, ty, req) in p {
            if s.is_empty() {
                conf.push(format!(
                    "A1 {} {nargs} {ty} {}",
                    hex(n.as_bytes()),
                    i32::from(req)
                ));
            } else {
                conf.push(format!(
                    "A2 {} {} {nargs} {ty} {}",
                    hex(s.as_bytes()),
                    hex(n.as_bytes()),
                    i32::from(req)
                ));
                keys.push(s.into());
            }
            keys.push(n.into());
            kinds.push((n.trim_start_matches('-').into(), tc(nargs, ty)));
            let k = if !s.is_empty() && g.chance(2) { s } else { n };
            args.push((k.into(), nargs, ty, req));
        }
    } else {
        for _ in 0..g.int(1, 5) {
            let nargs = g.pick(&[0i8, 1, 1, 2, 3, PLUS, STAR, -1]);
            let ty = g.pick(&[0i64, 0, 1, 2, 3, 4]);
            let req = g.chance(4);
            let n = g.pick(&NAMES);
            if g.chance(2) && n.starts_with("--") {
                let s = g.pick(&["-i", "-o", "-t", "-x", "-h", "-q", "a"]);
                conf.push(format!(
                    "A2 {} {} {nargs} {ty} {}",
                    hex(s.as_bytes()),
                    hex(n.as_bytes()),
                    i32::from(req)
                ));
                keys.push(s.into());
            } else {
                conf.push(format!(
                    "A1 {} {nargs} {ty} {}",
                    hex(n.as_bytes()),
                    i32::from(req)
                ));
            }
            keys.push(n.into());
            kinds.push((n.trim_start_matches('-').into(), tc(nargs, ty)));
            args.push((n.into(), nargs, ty, req));
        }
    }
    if help > 0 && !conf.iter().any(|c| c.starts_with('H')) {
        conf.push(format!("H {help}"));
    }
    t.push(conf.len().to_string());
    for c in conf {
        t.extend(c.split(' ').map(str::to_owned));
    }
    let mut argv: Vec<String> = vec![
        g.pick(&["prog", "/usr/bin/prog", "a/b/", "", "x/y/z"])
            .into(),
    ];
    if g.chance(2) {
        // a command line made for the configuration: each argument (the required ones nearly
        // always) with as many inputs of its type as it takes, now and then a stray word
        for (k, nargs, ty, req) in &args {
            if !(*req && !g.chance(12) || g.chance(2)) {
                continue;
            }
            if !k.starts_with('-') && !k.is_empty() {
                // a positional argument: its inputs only
            } else {
                argv.push(k.clone());
            }
            let n = match *nargs {
                PLUS => g.int(1, 3),
                STAR => g.int(0, 3),
                n if n < 0 => 1,
                n => i64::from(n),
            };
            for _ in 0..n {
                let v: String = match ty {
                    3 => g.pick(&["4", "-7", "0", " 12", "+3", "2147483647"]).into(),
                    4 => g
                        .pick(&["1.5", "-0.25", "3", "1e3", "0x1p-3", "nan(5)", "-inf"])
                        .into(),
                    1 => g.pick(&["true", "No", "1", "yes"]).into(),
                    _ => g.pick(&["in.mgz", "out dir/x", "a", "--", "-x"]).into(),
                };
                argv.push(if g.chance(15) { word(g) } else { v });
            }
            if g.chance(10) {
                argv.push(word(g));
            }
        }
        if g.chance(8) {
            argv.push("--version".into());
        }
    } else {
        for _ in 0..g.int(0, 7) {
            let a = match g.below(10) {
                0..=3 if !keys.is_empty() => keys[g.below(keys.len() as u64) as usize].clone(),
                4 => g
                    .pick(&["-q", "--zzz", "-h", "--help", "--version", "-"])
                    .into(),
                _ => word(g),
            };
            argv.push(a);
        }
    }
    t.push(argv.len().to_string());
    t.extend(argv.iter().map(|a| hex(a.as_bytes())));
    let nq = g.int(0, 6);
    t.push(nq.to_string());
    for _ in 0..nq {
        let (name, kind) = if !kinds.is_empty() && !g.chance(6) {
            kinds[g.below(kinds.len() as u64) as usize].clone()
        } else {
            (
                g.pick(&["nope", "version", "--version", "help"]).into(),
                'b',
            )
        };
        let name = if g.chance(4) && !name.is_empty() {
            format!("--{name}")
        } else {
            name
        };
        if g.chance(3) {
            t.push("E".into());
            t.push(hex(name.as_bytes()));
        } else {
            t.push("R".into());
            t.push(hex(name.as_bytes()));
            let k = if g.chance(5) {
                g.pick(&['i', 'f', 'b', 's', 'I', 'F', 'B', 'S'])
            } else {
                kind
            };
            t.push(k.to_string());
        }
    }
    t
}

// ---------------------------------------------------------------- the port's answers

thread_local! {
    /// Whether the XML help renderer (not ported) was asked to render a text.
    static RENDERED: Cell<bool> = const { Cell::new(false) };
    /// Whether FreeSurfer's answer depends on its name index's order (libstdc++'s hash order):
    /// two or more names matched where it takes the first.
    static AMBIG: Cell<bool> = const { Cell::new(false) };
}

/// Notes an order-dependent match: `n` names matched where FreeSurfer takes the first.
fn ambig_if(n: usize) {
    if n >= 2 {
        AMBIG.with(|a| a.set(true));
    }
}

/// The names that strip to `name` (`unstrip`'s candidates).
fn unstrip_matches(p: &ArgumentParser, name: &str) -> usize {
    p.keys()
        .iter()
        .filter(|k| ArgumentParser::strip(k) == name)
        .count()
}

/// The names a new argument's duplicate check matches.
fn dup_matches(p: &ArgumentParser, short: &str, name: &str) -> usize {
    let (s, n) = (ArgumentParser::strip(short), ArgumentParser::strip(name));
    p.keys()
        .iter()
        .filter(|k| {
            let t = ArgumentParser::strip(k);
            t == s || t == n
        })
        .count()
}

/// `addArgument(name)`'s (short, name) pair.
fn split_name(n: &str) -> (&str, &str) {
    if !n.starts_with('-') || n.len() > 2 {
        ("", n)
    } else {
        (n, "")
    }
}

fn help(g: &mut Globals, out: &mut dyn Write, err: &mut dyn Write, text: Option<&[u8]>) -> i32 {
    if matches!(text, Some(t) if !t.is_empty()) {
        RENDERED.with(|r| r.set(true));
    }
    argparse::output_help_xml(g, out, err, text)
}

const HELP_XML: &[u8] = b"<help><name>probe</name><synopsis>probe [options]</synopsis></help>";

fn of(o: &mut Out, f: f32) {
    o.x(&f.to_bits().to_be_bytes(), 4);
}

fn ostr(o: &mut Out, s: &str) {
    o.0.push('x');
    o.bytes(s.as_bytes());
    if s.is_empty() {
        o.0.truncate(o.0.len() - 2);
        o.0.push(' ');
    }
}

fn arg_type(v: i64) -> ArgType {
    match v {
        1 => ArgType::Bool,
        2 => ArgType::String,
        3 => ArgType::Int,
        4 => ArgType::Float,
        _ => ArgType::Unknown,
    }
}

pub fn run(
    tk: &[&str],
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    o: &mut Out,
    _w: &Work,
    extra: &mut Vec<&'static str>,
) -> Result<(), u8> {
    RENDERED.with(|r| r.set(false));
    AMBIG.with(|a| a.set(false));
    let r = answer(tk, g, out, err, o);
    if RENDERED.with(Cell::get) {
        // FreeSurfer's XML help renderer is not ported yet
        extra.push("SKIP");
    }
    if AMBIG.with(Cell::get) {
        extra.push("VARY");
    }
    r
}

fn answer(
    tk: &[&str],
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    o: &mut Out,
) -> Result<(), u8> {
    let st = |f: larmorx_freesurfer_core::log::Fatal| f.status();
    let mut i = 1;
    let mut next = || {
        let s = tk[i];
        i += 1;
        s
    };
    let text = |s: &str| String::from_utf8(super::unhex(s)).unwrap();
    let mut p = ArgumentParser::new(g, err).map_err(st)?;
    let nconf: usize = next().parse().unwrap();
    for _ in 0..nconf {
        match next() {
            "H" => {
                let mode = next();
                let t: &[u8] = if mode == "1" { b"" } else { HELP_XML };
                ambig_if(dup_matches(&p, "-h", "--help"));
                p.add_help(g, err, t).map_err(st)?;
            }
            "A1" => {
                let n = text(next());
                let nargs = next().parse::<i64>().unwrap() as i8;
                let ty = arg_type(next().parse().unwrap());
                let req = next() != "0";
                let (a, b) = split_name(&n);
                ambig_if(dup_matches(&p, a, b));
                p.add_argument(g, err, &n, nargs, ty, req).map_err(st)?;
            }
            _ => {
                let s = text(next());
                let n = text(next());
                let nargs = next().parse::<i64>().unwrap() as i8;
                let ty = arg_type(next().parse().unwrap());
                let req = next() != "0";
                ambig_if(dup_matches(&p, &s, &n));
                p.add_argument2(g, err, &s, &n, nargs, ty, req)
                    .map_err(st)?;
            }
        }
    }
    let nargv: usize = next().parse().unwrap();
    let argv: Vec<String> = (0..nargv).map(|_| text(next())).collect();
    o.0.push_str("parsed ");
    let env = Environment::default();
    let parsed = p.parse(g, &env, out, err, &argv, help);
    // the names parse looks up by unstrip (the required ones' too)
    for k in p.keys().iter().map(|k| ArgumentParser::strip(k).to_owned()) {
        ambig_if(unstrip_matches(&p, &k));
    }
    // --all-info's report holds the time (an unknown flag a positional argument's look-ahead
    // added to the index points at --all-info: a negative number given to it sets it too)
    let mut sink = Vec::new();
    if p.exists(g, &mut sink, "all-info").unwrap_or(false) {
        ambig_if(2);
    }
    parsed.map_err(st)?;
    let nq: usize = next().parse().unwrap();
    for _ in 0..nq {
        let k = next();
        let name = text(next());
        ambig_if(unstrip_matches(&p, &name));
        if k == "E" {
            o.i(i64::from(p.exists(g, err, &name).map_err(st)?));
            continue;
        }
        let want = match next() {
            "i" => "int",
            "f" => "float",
            "b" => "bool",
            "s" => "std::string",
            "I" => "std::vector<int>",
            "F" => "std::vector<float>",
            "B" => "std::vector<bool>",
            _ => "std::vector<std::string>",
        };
        match p.retrieve(g, err, &name, want).map_err(st)? {
            Value::Int(v) => o.i(i64::from(v)),
            Value::Float(v) => of(o, v),
            Value::Bool(v) => o.i(i64::from(v)),
            Value::Str(v) => ostr(o, &v),
            Value::Ints(v) => {
                o.i(v.len() as i64);
                v.iter().for_each(|&x| o.i(i64::from(x)));
            }
            Value::Floats(v) => {
                o.i(v.len() as i64);
                v.iter().for_each(|&x| of(o, x));
            }
            Value::Bools(v) => {
                o.i(v.len() as i64);
                v.iter().for_each(|&x| o.i(i64::from(x)));
            }
            Value::Strs(v) => {
                o.i(v.len() as i64);
                v.iter().for_each(|x| ostr(o, x));
            }
        }
    }
    Ok(())
}
