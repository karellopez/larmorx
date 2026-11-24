// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mergeseg`'s script (`scripts/mergeseg`, 303 lines of tcsh): the program's entry point
//! [`run`] (registered as `mergeseg` in the crate's `PROGRAMS`) and [`main`].
//!
//! `mergeseg --src <seg> --merge <seg> --o <out> [options]` merges one segmentation into
//! another: the source where the merge volume is 0, the merge volume's value elsewhere (with
//! `--segid <id>`, `<id>` there; with `--segid-only <id>`, only the merge volume's voxels of value
//! `<id>`, eroded `--segid-erode` times). The steps: `mri_binarize --inv` of the merge volume,
//! `mri_mask` of the source with it, `mri_binarize --binval` of the merge volume (with a segid),
//! `fscalc <merge> sum <masked source> -o <out>`, and with `--ctab` `mri_convert <out> --ctab
//! <ctab> <out>`, in a temporary folder (`fs_temp_dir` in the output's folder; `--tmpdir`) removed
//! at the end unless `--nocleanup` or `--tmpdir`; their command lines and outputs on standard
//! output and in `<out>.log` (`--log`, `--nolog`). `FS_MRIMASK_ALLOW_DIFF_GEOM` is 0 for its
//! programs.
//!
//! FreeSurfer's defects, corrected by default: FS-1274 (`-help`, `-version` inside an argument),
//! FS-1277 (names read again as shell words; not reproduced: a notice), FS-1278 (lines through
//! tcsh's `echo`), FS-1341 (no `SUBJECTS_DIR`: the script stops before the merge).

use larmorx_core::defects::FS_1341;
use larmorx_freesurfer_core::run::Settings;

use std::io::Write;

use crate::script::{Flow, Log, Sh, run_time_hours};
use crate::seg2cc::main::{asks, debug_line, dollar0};

/// The program's name as `argv[0]` gives it.
pub const NAME: &str = "mergeseg";
const VERSION: &str = "mergeseg 8.2.0";
const HELP: &str = include_str!("help.txt");

#[derive(Clone, Debug, Default)]
struct Opts {
    outseg: Option<String>,
    mergeseg: Option<String>,
    srcseg: Option<String>,
    segid: Option<String>,
    segidonly: bool,
    segiderode: Option<String>,
    ctab: Option<String>,
    tmpdir: Option<String>,
    lf: Option<String>,
    nocleanup: bool,
}

/// `mergeseg`'s entry point.
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
        "mergeseg --help",
        " --src source segmentation",
        " --merge merge seg volume ",
        " --o outseg",
        " --segid segindex (optional)",
        " --segid-only segindex : only take segindex from merge",
        " --segid-erode Nerode : erode seg-only segindex before merge",
        " --ctab ctab : color table to embed in the output",
        "",
    ] {
        sh.say(l);
    }
    if help {
        sh.echo(&[VERSION.to_owned()]);
        let _ = sh.out.write_all(HELP.as_bytes());
    }
}

/// `if($v > 0)` as tcsh 6.21 evaluates it for one word `v` (measured): digits (a decimal
/// number, wrapping as a 64-bit integer; a leading `0` is not octal), `-` and digits; a word
/// starting with a digit and holding anything else `if: Badly formed number.`, any other word
/// `if: Expression Syntax.` (both end the script, status 1); an empty word is false.
pub fn tcsh_positive(v: &str) -> Result<bool, &'static str> {
    if v.is_empty() {
        return Ok(false);
    }
    let (neg, digits) = match v.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, v),
    };
    let first = digits.chars().next();
    if !first.is_some_and(|c| c.is_ascii_digit()) {
        return Err("if: Expression Syntax.");
    }
    if !digits.chars().all(|c| c.is_ascii_digit()) {
        return Err("if: Badly formed number.");
    }
    let mut n: i64 = 0;
    for c in digits.bytes() {
        n = n.wrapping_mul(10).wrapping_add(i64::from(c - b'0'));
    }
    Ok(if neg { n.wrapping_neg() > 0 } else { n > 0 })
}

/// The script on `args`.
pub fn main(sh: &mut Sh, args: &[String]) -> Flow {
    for a in args {
        sh.check_name("the argument", a);
    }
    sh.env.set("FS_MRIMASK_ALLOW_DIFF_GEOM", "0");
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
    // source $FREESURFER_HOME/sources.csh
    let fsh = sh.var_or_stop("FREESURFER_HOME")?;
    sh.check_name("FREESURFER_HOME", &fsh);
    let sources = format!("{fsh}/sources.csh");
    if !sh.exists(&sources) {
        return sh.stop(&format!("{sources}: No such file or directory."));
    }
    let mut o = Opts::default();
    parse_args(sh, args, &mut o)?;
    let Some(srcseg) = o.srcseg.clone() else {
        sh.say("ERROR: must spec src seg");
        return Err(1);
    };
    let Some(mergeseg) = o.mergeseg.clone() else {
        sh.say("ERROR: must spec merge seg");
        return Err(1);
    };
    let Some(outseg) = o.outseg.clone() else {
        sh.say("ERROR: must spec out seg");
        return Err(1);
    };
    let cleanup = !o.nocleanup;
    let start_time = sh.date();
    let t_start = larmorx_freesurfer_core::ctime::now();
    // set outdir = `dirname $outseg`; mkdir -p; pushd; set outdir = `pwd`; popd
    let outdir = crate::csh::dirname(&outseg);
    sh.mkdir_p(&outdir);
    sh.pushd(&outdir, false)?;
    let outdir = sh.pwd();
    sh.check_name("the output's folder", &outdir);
    sh.popd(false)?;
    let tmpdir = match &o.tmpdir {
        Some(t) => t.clone(),
        None => {
            let (st, d) = sh.fs_temp_dir(&outdir);
            if st != 0 {
                // `set tmpdir = ...` of no words: `mkdir -p` of nothing
                let _ = writeln!(sh.err, "mkdir: missing operand");
                let _ = writeln!(sh.err, "Try 'mkdir --help' for more information.");
                String::new()
            } else {
                d
            }
        }
    };
    if !tmpdir.is_empty() {
        sh.mkdir_p(&tmpdir);
    }
    let lf = Log {
        name: o.lf.clone().unwrap_or_else(|| format!("{outseg}.log")),
        fixed: None,
    };
    if lf.name != "/dev/null" {
        let n = lf.name.clone();
        sh.rm_f(&n);
    }
    sh.append_say(&lf, "Log file for mergeseg")?;
    let d = sh.date();
    sh.tee_say(&lf, &d);
    sh.tee_say(&lf, "");
    // FS-1341: `echo "setenv SUBJECTS_DIR $SUBJECTS_DIR"` without SUBJECTS_DIR ends the script
    let sd = match sh.var("SUBJECTS_DIR") {
        Some(v) => v,
        None if sh.reproduce() => {
            sh.notice(
                &FS_1341,
                "mergeseg: SUBJECTS_DIR is not set: 'Undefined variable.', as in FreeSurfer",
            );
            return sh.stop("SUBJECTS_DIR: Undefined variable.");
        }
        None => {
            sh.notice(
                &FS_1341,
                "mergeseg: SUBJECTS_DIR is not set: its log line left empty (FreeSurfer stops with 'Undefined variable.')",
            );
            String::new()
        }
    };
    sh.tee_say(&lf, &format!("setenv SUBJECTS_DIR {sd}"));
    let pwd = sh.pwd();
    sh.tee_say(&lf, &format!("cd {pwd}"));
    let mut words = vec![dollar0(sh, &fsh, NAME)];
    words.extend(args.iter().cloned());
    sh.tee_echo(&lf, &words);
    sh.tee_say(&lf, "");
    let (_, stamp) = sh.cat(&format!("{fsh}/build-stamp.txt"));
    sh.tee(&lf, &stamp);
    sh.tee_echo(&lf, &[VERSION.to_owned()]);
    let u = sh.uname_a();
    sh.tee_say(&lf, &u);
    // the binarizations' options
    let segid_opts = |sh: &mut Sh, cmd: &mut Vec<String>| -> Flow {
        if o.segidonly {
            cmd.push("--match".into());
            cmd.push(o.segid.clone().unwrap_or_default());
            let e = o.segiderode.clone().unwrap_or_else(|| "0".into());
            match tcsh_positive(&e) {
                Err(msg) => return sh.stop(msg),
                Ok(true) => {
                    cmd.push("--erode".into());
                    cmd.push(e);
                }
                Ok(false) => {}
            }
        } else {
            cmd.push("--min".into());
            cmd.push("0.5".into());
        }
        Ok(())
    };
    let notmergeseg = format!("{tmpdir}/not.mergeseg.mgh");
    let mut cmd: Vec<String> = vec![
        "mri_binarize".into(),
        "--i".into(),
        mergeseg.clone(),
        "--inv".into(),
        "--o".into(),
        notmergeseg.clone(),
    ];
    segid_opts(sh, &mut cmd)?;
    step(sh, &lf, &cmd)?;
    let srcsegnotmerge = format!("{tmpdir}/srcsegnotmerge.mgh");
    let cmd = vec![
        "mri_mask".into(),
        srcseg.clone(),
        notmergeseg.clone(),
        srcsegnotmerge.clone(),
    ];
    step(sh, &lf, &cmd)?;
    if cleanup {
        sh.rm(&notmergeseg);
    }
    let mergeseg2 = match &o.segid {
        Some(id) => {
            let m2 = format!("{tmpdir}/mergeseg.mgh");
            let mut cmd: Vec<String> = vec![
                "mri_binarize".into(),
                "--i".into(),
                mergeseg.clone(),
                "--o".into(),
                m2.clone(),
                "--binval".into(),
                id.clone(),
            ];
            segid_opts(sh, &mut cmd)?;
            step(sh, &lf, &cmd)?;
            m2
        }
        None => mergeseg.clone(),
    };
    let cmd = vec![
        "fscalc".into(),
        mergeseg2,
        "sum".into(),
        srcsegnotmerge,
        "-o".into(),
        outseg.clone(),
    ];
    step(sh, &lf, &cmd)?;
    if let Some(ctab) = &o.ctab {
        let cmd = vec![
            "mri_convert".into(),
            outseg.clone(),
            "--ctab".into(),
            ctab.clone(),
            outseg.clone(),
        ];
        step(sh, &lf, &cmd)?;
    }
    if cleanup && !tmpdir.is_empty() {
        sh.rm_rf(&tmpdir);
    }
    let secs = larmorx_freesurfer_core::ctime::now() - t_start;
    sh.tee_say(&lf, " ");
    sh.tee_say(&lf, &format!("Started at {start_time} "));
    let d = sh.date();
    sh.tee_say(&lf, &format!("Ended   at {d}"));
    sh.tee_say(&lf, &format!("Mergeseg-Run-Time-Sec {secs}"));
    sh.tee_say(
        &lf,
        &format!("Mergeseg-Run-Time-Hours {}", run_time_hours(secs)),
    );
    sh.tee_say(&lf, " ");
    sh.tee_say(&lf, "mergeseg Done");
    Ok(())
}

/// `echo $cmd | tee -a $LF; $cmd | tee -a $LF; if($status) goto error_exit;` (`error_exit`:
/// `ERROR:`, status 1).
fn step(sh: &mut Sh, lf: &Log, cmd: &[String]) -> Flow {
    sh.tee_echo(lf, cmd);
    let (status, out) = sh.run(&cmd[0], &cmd[1..]);
    let t = sh.tee(lf, &out);
    if status != 0 || t != 0 {
        sh.say("ERROR:");
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
            "--src" => {
                let v = value(sh)?;
                if !sh.exists(&v) {
                    sh.say(&format!("ERROR: cannot find {v}"));
                    return Err(1);
                }
                o.srcseg = Some(v);
            }
            "--merge" => {
                let v = value(sh)?;
                if !sh.exists(&v) {
                    sh.say(&format!("ERROR: cannot find {v}"));
                    return Err(1);
                }
                o.mergeseg = Some(v);
            }
            "--o" => o.outseg = Some(value(sh)?),
            "--i" | "--segid" => o.segid = Some(value(sh)?),
            "--segid-only" => {
                let v = value(sh)?;
                o.segidonly = true;
                o.segid = Some(v);
            }
            "--segid-erode" => o.segiderode = Some(value(sh)?),
            "--ctab" => o.ctab = Some(value(sh)?),
            "--log" => o.lf = Some(value(sh)?),
            "--nolog" | "--no-log" => o.lf = Some("/dev/null".into()),
            "--tmp" | "--tmpdir" => {
                o.tmpdir = Some(value(sh)?);
                o.nocleanup = true;
            }
            "--nocleanup" => o.nocleanup = true,
            "--cleanup" => o.nocleanup = false,
            "--debug" => debug_line(sh),
            _ => {
                let words: Vec<String> = ["ERROR:", "Flag", &flag, "unrecognized."]
                    .iter()
                    .map(|s| s.to_string())
                    .collect();
                sh.echo(&words);
                sh.echo(args);
                return Err(1);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_as_tcsh_reads_them() {
        assert_eq!(tcsh_positive("0"), Ok(false));
        assert_eq!(tcsh_positive("12"), Ok(true));
        assert_eq!(tcsh_positive("08"), Ok(true));
        assert_eq!(tcsh_positive("-3"), Ok(false));
        assert_eq!(tcsh_positive("1.5"), Err("if: Badly formed number."));
        assert_eq!(tcsh_positive("3a"), Err("if: Badly formed number."));
        assert_eq!(tcsh_positive("x"), Err("if: Expression Syntax."));
        assert_eq!(tcsh_positive("+2"), Err("if: Expression Syntax."));
        assert_eq!(tcsh_positive("9999999999999999999"), Ok(false));
        assert_eq!(tcsh_positive(""), Ok(false));
    }
}
