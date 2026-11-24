// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `seg2cc`'s script (`scripts/seg2cc`, 362 lines of tcsh): the program's entry point [`run`]
//! (registered as `seg2cc` in the crate's `PROGRAMS`) and [`main`].
//!
//! `seg2cc --s <subject> [options]` adds the corpus callosum's labels to a subject's
//! segmentation: `mri_info --conformed-to-min` on `<subject>/mri/<nocc>.mgz` (default
//! `aseg.auto_noCCseg`), then for a volume conformed to its smallest voxel `mri_cc -aseg
//! <nocc>.mgz -o <cc>.mgz -lta transforms/cc_up.lta <subject>` (default `<cc>` `aseg.auto`),
//! else the volume and `<norm>.mgz` conformed (`mri_convert --conform_min`, `mri_label2vol`),
//! `mri_cc -norm` there, the result mapped back (`mri_label2vol`) and its five callosum labels
//! (251-255) merged into the segmentation (`mri_binarize`, `mergeseg`). Each step only when
//! `UpdateNeeded` says its output is older than its inputs (or `--force-update`). Its messages,
//! the steps' command lines and outputs (through `fs_time`) on standard output and in
//! `<subject>/scripts/seg2cc.log` (`--log`, `--nolog`); a temporary folder
//! `<subject>/mri/tmp/tmpdir.seg2cc.<process>` (`/scratch/...` when that folder is writable;
//! `--tmpdir`), removed at the end unless `--nocleanup` or `--tmpdir`.
//!
//! The ported programs run in this process ([`crate::script::Sh::run`]). FreeSurfer's defects,
//! corrected by default (`reproduce`: FreeSurfer's behaviour):
//! - **FS-1274**: an argument holding `-help` (`-version`) anywhere prints the usage (the version).
//! - **FS-1275**: `--expert` always ends the script with tcsh's `Undefined variable.`.
//! - **FS-1276**: `--ctab` is ignored (the merge always gets `FreeSurferColorLUT.txt`).
//! - **FS-1277**: names read again as shell words (not reproduced: a notice).
//! - **FS-1278**: printed lines through tcsh's `echo` (`-n`, backslashes).
//! - **FS-1279**: a relative `--log` or `--tmpdir` read from two folders.
//! - **FS-1280**: a relative `SUBJECTS_DIR` not found by the programs run in `<subject>/mri`.
//!
//! Not reproduced (both modes alike): `--debug`'s trace of every tcsh command (a line says so),
//! `sources.csh`'s macOS branch (`SetUpFreeSurfer.csh`), the texts of `date` and `printf` in
//! locales other than C.

use larmorx_core::defects::{FS_1274, FS_1275, FS_1276, FS_1279, FS_1280};
use larmorx_freesurfer_core::run::Settings;

use std::io::Write;
use std::path::Path;

use crate::script::{Flow, Log, Sh, canonical, run_time_hours};

/// The program's name as `argv[0]` gives it (FreeSurfer's scripts run it by name).
pub const NAME: &str = "seg2cc";
const VERSION: &str = "seg2cc 8.2.0";
/// What the script prints after its usage with `--help`: its lines after `BEGINHELP`.
const HELP: &str = include_str!("help.txt");

/// The options and the script's variables.
#[derive(Clone, Debug)]
struct Opts {
    subject: Option<String>,
    ccname: String,
    normname: String,
    noccname: String,
    ctab: Option<String>,
    tmpdir: Option<String>,
    lf: Option<String>,
    cleanup: bool,
    force: bool,
    lh: bool,
    rh: bool,
}

impl Default for Opts {
    fn default() -> Opts {
        Opts {
            subject: None,
            ccname: "aseg.auto".into(),
            normname: "norm".into(),
            noccname: "aseg.auto_noCCseg".into(),
            ctab: None,
            tmpdir: None,
            lf: None,
            cleanup: true,
            force: false,
            lh: false,
            rh: false,
        }
    }
}

/// `seg2cc`'s entry point: its original arguments (without the program's name), the settings,
/// the two streams; the exit status. The notices of the run follow on standard error.
pub fn run(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mut sh = Sh::new(NAME, settings, out, err);
    let status = match main(&mut sh, args) {
        Ok(()) => 0,
        Err(s) => s,
    };
    sh.finish(status)
}

/// `set n = `echo $argv | grep -e -<word> | wc -l``: FreeSurfer asks whether the arguments'
/// echo holds `-<word>`; by default (FS-1274) whether one of them is `-<word>` or `--<word>`.
pub fn asks(sh: &mut Sh, args: &[String], word: &str) -> bool {
    // tcsh's echo of the words (on Unix with its backslash sequences)
    let echoed = if cfg!(unix) {
        String::from_utf8_lossy(&crate::csh::echo(args)).into_owned()
    } else {
        args.join(" ")
    };
    let tcsh = echoed.contains(&format!("-{word}"));
    let plain = args
        .iter()
        .any(|a| a == &format!("-{word}") || a == &format!("--{word}"));
    if tcsh != plain {
        let name = sh.name;
        sh.notice(
            &FS_1274,
            if sh.reproduce() {
                format!("{name}: an argument holds '-{word}': taken as -{word}, as in FreeSurfer")
            } else {
                format!(
                    "{name}: an argument holding '-{word}' is not taken as -{word} (FreeSurfer takes it)"
                )
            },
        );
        return if sh.reproduce() { tcsh } else { plain };
    }
    plain
}

/// The script's name as tcsh's `$0` holds it when FreeSurfer runs it from its installation.
pub fn dollar0(sh: &Sh, fsh: &str, name: &str) -> String {
    let _ = sh;
    format!("{fsh}/bin/{name}")
}

fn usage(sh: &mut Sh, o: &Opts, help: bool) {
    let lines = [
        String::new(),
        "seg2cc".into(),
        "  --s subject : output".into(),
        format!(
            "  --nocc noccsegvol : name of aseg-type volume with out CC (default is {}) ",
            o.noccname
        ),
        format!(
            "  --cc ccsegvol : output name of aseg-type volume with CC (default is {}) ",
            o.ccname
        ),
        format!(
            "  --normvol : name of norm volume (default is {})",
            o.normname
        ),
        "  --lh-only, --rh-only".into(),
        "  --ctab ctab  : ctab for the seg (will use embedded if there or FreeSurferColorLUT.txt if not spec)".into(),
        "  --force-update : force regeneration of files whether needed or not (default is --no-force-update)".into(),
        String::new(),
    ];
    for l in lines {
        sh.say(&l);
    }
    if help {
        sh.echo(&[VERSION.to_owned()]);
        let _ = sh.out.write_all(HELP.as_bytes());
    }
}

/// The script on `args`.
pub fn main(sh: &mut Sh, args: &[String]) -> Flow {
    for a in args {
        sh.check_name("the argument", a);
    }
    // if(-e $FREESURFER_HOME/sources.csh) source ...: nothing on Linux
    let fsh = sh.var_or_stop("FREESURFER_HOME")?;
    sh.check_name("FREESURFER_HOME", &fsh);
    let mut o = Opts::default();
    let ctabdefault = format!("{fsh}/FreeSurferColorLUT.txt");
    if args.is_empty() {
        usage(sh, &o, false);
        return Err(1);
    }
    if asks(sh, args, "help") {
        usage(sh, &o, true);
        return Err(1);
    }
    if asks(sh, args, "version") {
        sh.echo(&[VERSION.to_owned()]);
        return Err(0);
    }
    parse_args(sh, args, &mut o)?;
    // check_params
    let Some(subject) = o.subject.clone() else {
        sh.say("ERROR: must supply a subject name");
        return Err(1);
    };
    let sd = sh.var_or_stop("SUBJECTS_DIR")?;
    sh.check_name("SUBJECTS_DIR", &sd);
    sh.check_name("the folder", &sh.pwd());
    let mut segvol = format!("{sd}/{subject}/mri/{}.mgz", o.noccname);
    let mut normvol = format!("{sd}/{subject}/mri/{}.mgz", o.normname);
    let segvolcc = format!("{sd}/{subject}/mri/{}.mgz", o.ccname);
    for f in [&segvol, &normvol] {
        if !sh.exists(f) {
            let line = format!("ERROR: cannot find {f}");
            sh.say(&line);
            return Err(1);
        }
    }
    segvol = fullpath(sh, &segvol);
    normvol = fullpath(sh, &normvol);
    if !o.force {
        let (ud, _) = sh.update_needed(&segvolcc, &[segvol.clone(), normvol.clone()]);
        if ud == "0" {
            sh.say("  seg2cc: update not needed. Run with --force-update to force an update");
            return Err(0);
        }
    }
    // the folder seg2cc started in: relative names given to it are read from there by default
    let start = sh.cwd.clone();
    let fixed = !sh.reproduce();
    let start_time = sh.date();
    let t_start = larmorx_freesurfer_core::ctime::now();
    for d in ["scripts", "mri/transforms", "mri/tmp"] {
        sh.mkdir_p(&format!("{sd}/{subject}/{d}"));
    }
    let pid = std::process::id();
    let tmpdir = match &o.tmpdir {
        Some(t) => t.clone(),
        None if sh.dir_writable("/scratch") => format!("/scratch/tmpdir.seg2cc.{pid}"),
        None => format!("{sd}/{subject}/mri/tmp/tmpdir.seg2cc.{pid}"),
    };
    sh.mkdir_p(&tmpdir);
    // FS-1279: the temporary folder's and the log's names from the starting folder
    let tmpdir_rel = !Path::new(&tmpdir).is_absolute();
    let tmp_at = if fixed && tmpdir_rel {
        canonical(&start, &tmpdir).to_string_lossy().into_owned()
    } else {
        tmpdir.clone()
    };
    let lf_name =
        o.lf.clone()
            .unwrap_or_else(|| format!("{sd}/{subject}/scripts/seg2cc.log"));
    let lf_rel = lf_name != "/dev/null" && !Path::new(&lf_name).is_absolute();
    let lf = Log {
        fixed: (fixed && lf_rel).then(|| canonical(&start, &lf_name)),
        name: lf_name.clone(),
    };
    if lf_rel || tmpdir_rel {
        let what = match (lf_rel, tmpdir_rel) {
            (true, true) => "the log's and the temporary folder's relative names",
            (true, false) => "the log's relative name",
            _ => "the temporary folder's relative name",
        };
        sh.notice(
            &FS_1279,
            if fixed {
                format!("seg2cc: {what} read from the folder seg2cc started in (FreeSurfer reads them again inside <subject>/mri)")
            } else {
                format!("seg2cc: {what} read again inside <subject>/mri, as in FreeSurfer")
            },
        );
    }
    // FS-1280: the programs run inside <subject>/mri get an absolute subjects folder
    if !Path::new(&sd).is_absolute() {
        if fixed {
            let abs = canonical(&start, &sd);
            sh.env.set("SUBJECTS_DIR", abs.as_os_str());
            sh.notice(
                &FS_1280,
                "seg2cc: the relative SUBJECTS_DIR given to the programs as an absolute name (FreeSurfer's programs read it inside <subject>/mri)",
            );
        } else {
            sh.notice(
                &FS_1280,
                "seg2cc: the relative SUBJECTS_DIR read by the programs inside <subject>/mri, as in FreeSurfer",
            );
        }
    }
    if lf.name != "/dev/null" {
        let at = lf.fixed.clone().map(|p| p.to_string_lossy().into_owned());
        sh.rm_f(at.as_deref().unwrap_or(&lf.name));
    }
    sh.append_say(&lf, "Log file for seg2cc")?;
    let d = sh.date();
    sh.tee_say(&lf, &d);
    sh.tee_say(&lf, "");
    let sdv = sh.var("SUBJECTS_DIR").unwrap_or_default();
    let shown_sd = if fixed && !Path::new(&sd).is_absolute() {
        sd.clone()
    } else {
        sdv
    };
    sh.tee_say(&lf, &format!("setenv SUBJECTS_DIR {shown_sd}"));
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
    if let Some(job) = sh.var("PBS_JOBID") {
        sh.append_say(&lf, &format!("pbsjob {job}"))?;
    }
    let mdir = format!("{sd}/{subject}/mri");
    sh.pushd(&mdir, true)?;
    let tmpfile = format!("{tmp_at}/isconformed");
    let (_, _) = sh.run(
        "mri_info",
        &[
            "--o".into(),
            tmpfile.clone(),
            "--conformed-to-min".into(),
            segvol.clone(),
        ],
    );
    let (_, text) = sh.cat(&tmpfile);
    let first = text.split(|&b| b == b'\n').next().unwrap_or(&[]);
    let isconf: Vec<String> = String::from_utf8_lossy(first)
        .split_ascii_whitespace()
        .map(str::to_owned)
        .collect();
    let mut line = vec!["IsConformed".to_owned()];
    line.extend(isconf.iter().cloned());
    sh.tee_echo(&lf, &line);
    let hemis = |cmd: &mut Vec<String>| {
        if o.lh {
            cmd.push("-lh".into());
        }
        if o.rh {
            cmd.push("-rh".into());
        }
    };
    if isconf.len() > 1 {
        return sh.stop("if: Expression Syntax.");
    }
    if isconf.first().map(String::as_str) == Some("yes") {
        let (ud, _) = sh.update_needed(
            &format!("{}.mgz", o.ccname),
            &[format!("{}.mgz", o.noccname)],
        );
        if ud == "1" || o.force {
            let mut cmd: Vec<String> = [
                "mri_cc",
                "-aseg",
                &format!("{}.mgz", o.noccname),
                "-o",
                &format!("{}.mgz", o.ccname),
                "-lta",
                "transforms/cc_up.lta",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect();
            hemis(&mut cmd);
            cmd.push(subject.clone());
            let d = sh.date();
            sh.tee_say(&lf, &d);
            step(sh, &lf, &cmd)?;
            let d = sh.date();
            sh.tee_say(&lf, &d);
        }
    } else {
        let normconf = format!("{}.conf.mgz", o.normname);
        let (ud, _) = sh.update_needed(&normconf, std::slice::from_ref(&normvol));
        if ud == "1" || o.force {
            let cmd = vec![
                "mri_convert".into(),
                normvol.clone(),
                normconf.clone(),
                "--conform_min".into(),
            ];
            step(sh, &lf, &cmd)?;
        }
        let nocc_conf = format!("{}.conf.mgz", o.noccname);
        let (ud, _) = sh.update_needed(&nocc_conf, &[normconf.clone(), segvol.clone()]);
        if ud == "1" || o.force {
            let cmd = vec![
                "mri_label2vol".into(),
                "--seg".into(),
                segvol.clone(),
                "--temp".into(),
                normconf.clone(),
                "--regheader".into(),
                "--o".into(),
                nocc_conf.clone(),
            ];
            step(sh, &lf, &cmd)?;
        }
        let cc_conf = format!("{}.conf.mgz", o.ccname);
        let (ud, _) = sh.update_needed(&cc_conf, std::slice::from_ref(&nocc_conf));
        if ud == "1" || o.force {
            let mut cmd: Vec<String> = vec![
                "mri_cc".into(),
                "-norm".into(),
                normconf.clone(),
                "-aseg".into(),
                nocc_conf.clone(),
                "-o".into(),
                cc_conf.clone(),
                "-lta".into(),
                "transforms/cc_up.lta".into(),
            ];
            hemis(&mut cmd);
            cmd.push(subject.clone());
            step(sh, &lf, &cmd)?;
        }
        let cc_noconf = format!("{}.noconf.mgz", o.ccname);
        let cmd = vec![
            "mri_label2vol".into(),
            "--seg".into(),
            cc_conf.clone(),
            "--temp".into(),
            normvol.clone(),
            "--regheader".into(),
            "--o".into(),
            cc_noconf.clone(),
        ];
        step(sh, &lf, &cmd)?;
        // FS-1276: --ctab's table for the merge
        let ctab = match &o.ctab {
            Some(c) if fixed => {
                sh.notice(
                    &FS_1276,
                    format!("seg2cc: the merge gets --ctab's {c} (FreeSurfer ignores --ctab)"),
                );
                c.clone()
            }
            Some(c) => {
                sh.notice(
                    &FS_1276,
                    format!("seg2cc: --ctab {c} ignored, as in FreeSurfer"),
                );
                ctabdefault.clone()
            }
            None => ctabdefault.clone(),
        };
        for segid in 251..=255 {
            let cmd = vec![
                "mri_binarize".into(),
                "--i".into(),
                cc_noconf.clone(),
                "--match".into(),
                segid.to_string(),
                "--o".into(),
                format!("{tmp_at}/ccbin.mgh"),
            ];
            step(sh, &lf, &cmd)?;
            let src = if segid == 251 {
                format!("{}.mgz", o.noccname)
            } else {
                format!("{}.mgz", o.ccname)
            };
            let mut cmd = vec![
                "mergeseg".into(),
                "--src".into(),
                src,
                "--merge".into(),
                format!("{tmp_at}/ccbin.mgh"),
                "--o".into(),
                format!("{}.mgz", o.ccname),
                "--segid".into(),
                segid.to_string(),
                "--tmpdir".into(),
                format!("{tmp_at}/tmp.mergeseg"),
                "--ctab".into(),
                ctab.clone(),
            ];
            if o.cleanup {
                cmd.push("--cleanup".into());
            }
            step(sh, &lf, &cmd)?;
        }
    }
    sh.popd(true)?;
    if o.cleanup {
        sh.rm_rf(&tmp_at);
    }
    let t_end = larmorx_freesurfer_core::ctime::now();
    let secs = t_end - t_start;
    sh.tee_say(&lf, " ");
    sh.tee_say(&lf, &format!("Started at {start_time} "));
    let d = sh.date();
    sh.tee_say(&lf, &format!("Ended   at {d}"));
    sh.tee_say(&lf, &format!("Seg2cc-Run-Time-Sec {secs}"));
    sh.tee_say(
        &lf,
        &format!("Seg2cc-Run-Time-Hours {}", run_time_hours(secs)),
    );
    sh.tee_say(&lf, " ");
    sh.tee_say(&lf, "seg2cc Done");
    Ok(())
}

/// `echo $cmd | tee -a $LF; fs_time $cmd | tee -a $LF; if($status) goto error_exit;`
/// (`error_exit`: `ERROR: <cmd>`, status 1).
fn step(sh: &mut Sh, lf: &Log, cmd: &[String]) -> Flow {
    sh.tee_echo(lf, cmd);
    let (status, out) = sh.fs_time(&cmd[0], &cmd[1..]);
    let t = sh.tee(lf, &out);
    if status != 0 || t != 0 {
        let mut words = vec!["ERROR:".to_owned()];
        words.extend(cmd.iter().cloned());
        let line = words.join(" ");
        sh.say(&line);
        return Err(1);
    }
    Ok(())
}

/// `` `getfullpath <name>` `` (the ported script): its output's line.
fn fullpath(sh: &mut Sh, name: &str) -> String {
    let (_, out) = sh.run("getfullpath", &[name.to_owned()]);
    String::from_utf8_lossy(&out)
        .trim_end_matches('\n')
        .to_owned()
}

/// `parse_args`.
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
            "--cc" => o.ccname = value(sh)?,
            "--nocc" => o.noccname = value(sh)?,
            "--s" => o.subject = Some(value(sh)?),
            "--n" => o.normname = value(sh)?,
            "--ctab" => o.ctab = Some(value(sh)?),
            "--threads" => {
                value(sh)?;
            }
            "--expert" => {
                let x = value(sh)?;
                expert(sh, &x)?;
            }
            "-lh" | "-lh-only" | "--lh-only" => o.lh = true,
            "-rh" | "-rh-only" | "--rh-only" => o.rh = true,
            "--log" => o.lf = Some(value(sh)?),
            "--nolog" | "--no-log" => o.lf = Some("/dev/null".into()),
            "--no-force-update" => o.force = false,
            "--force-update" => o.force = true,
            "--tmp" | "--tmpdir" => {
                o.tmpdir = Some(value(sh)?);
                o.cleanup = false;
            }
            "--nocleanup" => o.cleanup = false,
            "--cleanup" => o.cleanup = true,
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

/// `--debug`: tcsh's trace of every command (`set echo`, `set verbose`) is not reproduced.
pub fn debug_line(sh: &mut Sh) {
    let name = sh.name;
    let _ = writeln!(
        sh.err,
        "larmorx: {name} --debug: tcsh's trace of each command it runs is not printed (not reproduced)"
    );
}

/// `--expert <file>` (FS-1275): `fsr-checkxopts`, then FreeSurfer's `XOptsFiles: Undefined
/// variable.` (or after a failed check `cmd: Undefined variable.`); by default the check alone.
fn expert(sh: &mut Sh, file: &str) -> Flow {
    let st = sh.checkxopts(file);
    if sh.reproduce() {
        sh.notice(
            &FS_1275,
            "seg2cc --expert: tcsh's 'Undefined variable.', as in FreeSurfer",
        );
        if st != 0 {
            return sh.stop("cmd: Undefined variable.");
        }
        return sh.stop("XOptsFiles: Undefined variable.");
    }
    sh.notice(
        &FS_1275,
        "seg2cc --expert: the file checked and not used (FreeSurfer ends with 'Undefined variable.')",
    );
    if st != 0 {
        return Err(1);
    }
    Ok(())
}
