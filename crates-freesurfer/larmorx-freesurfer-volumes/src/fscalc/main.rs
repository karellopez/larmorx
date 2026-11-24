// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `fscalc`'s script (`scripts/fscalc`, 342 lines of tcsh): the program's entry point [`run`]
//! (registered as `fscalc` in the crate's `PROGRAMS`) and [`main`].
//!
//! `fscalc <input> <op> [<input2>] [<op> ...] -o <out> [options]`: the first input copied into
//! a temporary volume (`mri_convert`, with `--float` as floats), each operation applied to it in
//! turn (`mris_calc -o <tmp> <tmp> <op> [<input>]`: binary operations take the next word, a
//! volume or a number; a leading `-` is dropped; `sum` is `add`), the result written (`mri_convert
//! <tmp> <out>`, with `--odt <type> --no_scale 1`). Every word that is not an option is an input
//! or an operation. In a temporary folder (`fs_temp_dir` in the output's folder; `--tmpdir`) with
//! its log `fscalc.log` (`--log`, `--nolog`), removed at the end unless `--nocleanup` or
//! `--tmpdir`; `--no-run` prints the command lines only. Its help ends with `mris_calc -u`'s.
//!
//! A first input that is not a file (a number) needs FreeSurfer's `mri_volsynth`, which is not
//! ported yet: the script stops there with a message (status 1).
//!
//! FreeSurfer's defects, corrected by default: FS-1274 (`-help`, `-version` inside an argument),
//! FS-1277 (names read again as shell words; not reproduced: a notice), FS-1278 (lines and
//! operations through tcsh's `echo`: an operation `-n`).

use larmorx_freesurfer_core::run::Settings;

use std::io::Write;

use crate::script::{Flow, Log, Sh};
use crate::seg2cc::main::{asks, debug_line, dollar0};

/// The program's name as `argv[0]` gives it.
pub const NAME: &str = "fscalc";
const VERSION: &str = "fscalc 8.2.0";
const HELP: &str = include_str!("help.txt");

const BINARY: [&str; 26] = [
    "mul", "div", "mod", "add", "sub", "sub0", "sqd", "atan2", "mag", "eq", "lt", "lte", "gt",
    "gte", "upl", "lrl", "and", "or", "andbw", "orbw", "masked", "pow", "bcor", "pctdiff",
    "pctdiff0", "sratio",
];
const UNARY: [&str; 10] = [
    "sqr", "sqrt", "abs", "log", "log10", "sign", "norm", "inv", "sig2p", "not",
];
/// `fname2ext`'s extensions.
const EXTENSIONS: [&str; 9] = [
    "mgh", "mgz", "nii", "nii.gz", "img", "bhdr", "annot", "m3z", "gii",
];

#[derive(Clone, Debug, Default)]
struct Opts {
    outvol: Option<String>,
    inops: Vec<String>,
    tmpdir: Option<String>,
    nocleanup: bool,
    lf: Option<String>,
    no_run: bool,
    odt: Option<String>,
    float: bool,
}

/// `fscalc`'s entry point.
pub fn run(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mut sh = Sh::new(NAME, settings, out, err);
    let status = match main(&mut sh, args) {
        Ok(()) => 0,
        Err(s) => s,
    };
    sh.finish(status)
}

fn usage(sh: &mut Sh, help: bool) {
    for l in [
        "",
        "fscalc input1 op <input2 op ...> --o outvol",
        "",
        "Binary Ops: mul, div, mod, sub, sub0, sratio, pctdiff, pctdiff0, sqd, atan2, mag, eq, ",
        " lt, lte, gt, gte, upl, lrl, and, or, andbw, orbw, masked, bcor",
        "",
        "Unary Ops: sqr, sqrt, abs, log, log10, sign, norm, sig2p, not",
        "",
        "--odt type : output data type (uchar|short|int|float).",
        "--help",
        "",
        " --debug",
        " --tmpdir tmpdir",
        " --nocleanup ",
        " --log logfile",
        "",
    ] {
        sh.say(l);
    }
    if help {
        sh.echo(&[VERSION.to_owned()]);
        let _ = sh.out.write_all(HELP.as_bytes());
        let (_, o) = sh.run("mris_calc", &["-u".into()]);
        let _ = sh.out.write_all(&o);
    }
}

/// `fname2ext <name>` succeeds: the name's last component ends with one of its extensions (and
/// is more than it).
fn has_extension(name: &str) -> bool {
    let base = crate::csh::basename(name);
    EXTENSIONS.iter().any(|e| {
        let suffix = format!(".{e}");
        base != suffix && base.ends_with(&suffix)
    })
}

/// The script on `args`.
pub fn main(sh: &mut Sh, args: &[String]) -> Flow {
    for a in args {
        sh.check_name("the argument", a);
    }
    if args.is_empty() {
        usage(sh, false);
        return Err(1);
    }
    if asks(sh, args, "help") {
        usage(sh, true);
        return Err(1);
    }
    if asks(sh, args, "version") {
        sh.echo(&[VERSION.to_owned()]);
        return Err(0);
    }
    let fsh = sh.var_or_stop("FREESURFER_HOME")?;
    sh.check_name("FREESURFER_HOME", &fsh);
    let sources = format!("{fsh}/sources.csh");
    if !sh.exists(&sources) {
        return sh.stop(&format!("{sources}: No such file or directory."));
    }
    let mut o = Opts::default();
    parse_args(sh, args, &mut o)?;
    let Some(outvol) = o.outvol.clone() else {
        sh.say("ERROR: must spec output");
        return Err(1);
    };
    if o.inops.len() < 2 {
        sh.say("ERROR: must spec an input and an operation");
        return Err(1);
    }
    let cleanup = !o.nocleanup;
    let outdir = crate::csh::dirname(&outvol);
    sh.mkdir_p(&outdir);
    let tmpdir = match &o.tmpdir {
        Some(t) => t.clone(),
        None => {
            let (st, d) = sh.fs_temp_dir(&outdir);
            if st != 0 { String::new() } else { d }
        }
    };
    if tmpdir.is_empty() {
        let _ = writeln!(sh.err, "mkdir: missing operand");
        let _ = writeln!(sh.err, "Try 'mkdir --help' for more information.");
    } else {
        sh.mkdir_p(&tmpdir);
    }
    let lf = Log {
        name: o
            .lf
            .clone()
            .unwrap_or_else(|| format!("{tmpdir}/fscalc.log")),
        fixed: None,
    };
    if lf.name != "/dev/null" {
        let n = lf.name.clone();
        sh.rm_f(&n);
    }
    sh.append_say(&lf, "Log file for fscalc")?;
    let d = sh.date();
    sh.tee_say(&lf, &d);
    sh.tee_say(&lf, "");
    let pwd = sh.pwd();
    sh.tee_say(&lf, &format!("cd {pwd}"));
    let mut words = vec![dollar0(sh, &fsh, NAME)];
    words.extend(args.iter().cloned());
    sh.tee_echo(&lf, &words);
    sh.tee_say(&lf, "");
    let (_, stamp) = sh.cat(&format!("{fsh}/build-stamp.txt"));
    sh.tee(&lf, &stamp);
    let u = sh.uname_a();
    sh.tee_say(&lf, &u);
    let tmpvol = format!("{tmpdir}/tmp.mgh");
    let mut inops = o.inops.clone();
    let i1 = inops.remove(0);
    let cmd: Vec<String> = if sh.exists(&i1) {
        let mut c = vec!["mri_convert".to_owned(), i1.clone(), tmpvol.clone()];
        if o.float {
            c.push("-odt".into());
            c.push("float".into());
        }
        c
    } else {
        if has_extension(&i1) {
            sh.say(&format!("ERROR: cannot find {i1}"));
            return Err(1);
        }
        let Some(template) = inops.iter().find(|w| sh.exists(w)).cloned() else {
            sh.say("ERROR: no inputs are files (that exist)");
            return Err(1);
        };
        vec![
            "mri_volsynth".into(),
            "--template".into(),
            template,
            "--pdf".into(),
            "const".into(),
            "--val-a".into(),
            i1.clone(),
            "--o".into(),
            tmpvol.clone(),
        ]
    };
    sh.tee_echo(&lf, &cmd);
    if !o.no_run {
        if cmd[0] == "mri_volsynth" {
            let _ = writeln!(
                sh.err,
                "larmorx: fscalc: a first input that is a number ({i1}) needs FreeSurfer's mri_volsynth, which is not ported yet"
            );
            return Err(1);
        }
        run_step(sh, &lf, &cmd)?;
    }
    while !inops.is_empty() {
        sh.tee_say(&lf, "----------------------");
        let mut op = inops.remove(0);
        // set c = `echo $op | cut -c1`; if("$c" == "-") set op = `echo $op | cut -c 2-`
        let echoed = sh.echo_bytes(std::slice::from_ref(&op));
        let body = echoed.strip_suffix(b"\n").unwrap_or(&echoed);
        let lines: Vec<&[u8]> = if echoed.is_empty() {
            Vec::new()
        } else {
            body.split(|&b| b == b'\n').collect()
        };
        let cut = |from: usize, to: Option<usize>| -> String {
            let parts: Vec<String> = lines
                .iter()
                .map(|l| {
                    let end = to.map_or(l.len(), |t| t.min(l.len()));
                    String::from_utf8_lossy(l.get(from.min(end)..end).unwrap_or(&[])).into_owned()
                })
                .collect();
            parts
                .join("\n")
                .split_ascii_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        };
        if cut(0, Some(1)) == "-" {
            op = cut(1, None);
        }
        if op == "sum" {
            op = "add".into();
        }
        let mut opok = false;
        if BINARY.contains(&op.as_str()) {
            opok = true;
            if inops.is_empty() {
                sh.tee_say(&lf, &format!("ERROR: {op} requires two arguments"));
                return Err(1);
            }
            let i2 = inops.remove(0);
            let cmd = vec![
                "mris_calc".to_owned(),
                "-o".into(),
                tmpvol.clone(),
                tmpvol.clone(),
                op.clone(),
                i2,
            ];
            sh.tee_echo(&lf, &cmd);
            if !o.no_run {
                run_step(sh, &lf, &cmd)?;
            }
        }
        if UNARY.contains(&op.as_str()) {
            opok = true;
            let cmd = vec![
                "mris_calc".to_owned(),
                "-o".into(),
                tmpvol.clone(),
                tmpvol.clone(),
                op.clone(),
            ];
            sh.tee_echo(&lf, &cmd);
            if !o.no_run {
                run_step(sh, &lf, &cmd)?;
            }
        }
        if !opok {
            sh.say(&format!("ERROR: operation {op} not recognized"));
            return Err(1);
        }
    }
    sh.tee_say(&lf, "----------------------");
    let mut cmd = vec!["mri_convert".to_owned(), tmpvol.clone(), outvol.clone()];
    if let Some(t) = &o.odt {
        cmd.extend([
            "-odt".to_owned(),
            t.clone(),
            "--no_scale".into(),
            "1".into(),
        ]);
    }
    sh.tee_echo(&lf, &cmd);
    if !o.no_run {
        run_step(sh, &lf, &cmd)?;
    }
    if cleanup && !tmpdir.is_empty() {
        sh.rm_rf(&tmpdir);
    }
    Ok(())
}

/// `$cmd | tee -a $LF; if($status) exit 1;`
fn run_step(sh: &mut Sh, lf: &Log, cmd: &[String]) -> Flow {
    let (status, out) = sh.run(&cmd[0], &cmd[1..]);
    let t = sh.tee(lf, &out);
    if status != 0 || t != 0 {
        return Err(1);
    }
    Ok(())
}

fn parse_args(sh: &mut Sh, args: &[String], o: &mut Opts) -> Flow {
    let mut i = 0;
    while i < args.len() {
        let flag = args[i].clone();
        i += 1;
        let mut value = |sh: &mut Sh| -> Flow<String> {
            match args.get(i) {
                Some(v) => {
                    i += 1;
                    Ok(v.clone())
                }
                None => {
                    sh.say(&format!("ERROR: flag {flag} requires one argument"));
                    Err(1)
                }
            }
        };
        match flag.as_str() {
            "-o" | "--o" => o.outvol = Some(value(sh)?),
            "-odt" | "--odt" => o.odt = Some(value(sh)?),
            "--float" | "-float" => o.float = true,
            "--log" => o.lf = Some(value(sh)?),
            "--nolog" | "--no-log" => o.lf = Some("/dev/null".into()),
            "--tmp" | "--tmpdir" => {
                o.tmpdir = Some(value(sh)?);
                o.nocleanup = true;
            }
            "--nocleanup" => o.nocleanup = true,
            "--cleanup" => o.nocleanup = false,
            "--no-run" => o.no_run = true,
            "--debug" => debug_line(sh),
            _ => o.inops.push(flag.clone()),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_as_fname2ext() {
        assert!(has_extension("a/x.mgz") && has_extension("x.nii.gz") && has_extension("x.nii"));
        assert!(!has_extension(".mgz") && !has_extension("100") && !has_extension("x.gz"));
    }
}
