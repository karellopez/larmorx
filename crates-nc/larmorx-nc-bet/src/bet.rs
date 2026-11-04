// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret (the translation).
//! The `bet` script (fsl-bet2 2111.8, `bet`, run by bash with `-e` and an `ERR` trap): its
//! options, messages and variations, with the programs it runs replaced by their replicas
//! (`bet2` here, the avwutils parts in [`avw`](crate::avw)).
//!
//! - default: `bet2` with the options passed on;
//! - `-R`: `bet2` again from the brain's centre of gravity (`fslstats -l 0.001 -C`) until it
//!   moves by less than one voxel (the distance's square truncated to an integer by `dc`) or 10
//!   repeats;
//! - `-Z`: the first and last slices repeated until the z field of view reaches 100 mm, `bet2`,
//!   then the outputs cropped back;
//! - `-F`: `bet2 -m -n -f 0.3` on the first volume, the mask dilated (`fslmaths -dilF`), every
//!   volume masked (`fslmaths -mas`);
//! - `-S`: `bet2 -s`, `standard_space_roi -b -d` and `bet2 -n -m -f 0.4` on its result, MNI152's
//!   eye mask taken into the input's space (`flirt -applyxfm` through standard_space_roi's
//!   inverse matrix) and dilated (`fslmaths`), the brain without the eyes and without what is
//!   brighter than 3.5 standard deviations above its mean (`fslstats -M`, `-S`, `dc`), the skull
//!   without the eyes;
//! - `-B`: `standard_space_roi -b -d`, FAST's bias field (`fast -b`, its messages discarded),
//!   the input divided by it, (with `-s` the skull from `standard_space_roi -maskFOV -roiNONE`
//!   and `bet2`, moved with fslpy's `immv`), `standard_space_roi -b -d` again and `bet -m -R -d`
//!   (this script again) on its result;
//! - `-A`: `flirt` to MNI152 (the matrix only), `bet2 -e`, `betsurf --t1only -o -m -s`;
//! - `-A2 <T2>`: `flirt` of the T2 to the input (`-interp sinc`), `flirt` to MNI152, `bet2 -e`,
//!   `betsurf -o -m -s` with the registered T2.
//!
//! Every program runs in this process (the replicas of FLIRT, standard_space_roi, FAST,
//! fslmaths, fslstats, betsurf); a failure stops the script through its `ERR` trap, as bash's
//! `-e` does. The standard images (`${FSLDIR}/data/standard/MNI152_T1_2mm`, its dilated brain
//! mask, the eye mask) are found by [`larmorx_nc_core::fsldata`]; the script refuses before it
//! runs anything when one it needs is nowhere.
//!
//! The script expands its arguments unquoted (FSL-44): the replica reproduces the word
//! splitting of blanks in `reproduce` mode and uses the names as given by default.

use std::io::Write;
use std::time::SystemTime;

use larmorx_core::defects::{self, Notices, UpstreamDefects};

use crate::bet2::{self, Run};
use larmorx_nc_core::avwutils::{self as avw, ToolFailure};
use larmorx_nc_core::exception;
use larmorx_nc_core::fsldata;
use larmorx_nc_core::fslpy;
use larmorx_nc_core::settings::Settings;

const USAGE: &str = "
Usage:    bet <input> <output> [options]

Main bet2 options:
  -o          generate brain surface outline overlaid onto original image
  -m          generate binary brain mask
  -s          generate approximate skull image
  -n          don't generate segmented brain image output
  -f <f>      fractional intensity threshold (0->1); default=0.5; smaller values give larger brain outline estimates
  -g <g>      vertical gradient in fractional intensity threshold (-1->1); default=0; positive values give larger brain outline at bottom, smaller at top
  -r <r>      head radius (mm not voxels); initial surface sphere is set to half of this
  -c <x y z>  centre-of-gravity (voxels not mm) of initial mesh surface.
  -t          apply thresholding to segmented brain image and mask
  -e          generates brain surface as mesh in .vtk format

Variations on default bet2 functionality (mutually exclusive options):
  (default)   just run bet2
  -R          robust brain centre estimation (iterates BET several times)
  -S          eye & optic nerve cleanup (can be useful in SIENA - disables -o option)
  -B          bias field & neck cleanup (can be useful in SIENA)
  -Z          improve BET if FOV is very small in Z (by temporarily padding end slices)
  -F          apply to 4D FMRI data (uses -f 0.3 and dilates brain mask slightly)
  -A          run bet2 and then betsurf to get additional skull and scalp surfaces (includes registrations)
  -A2 <T2>    as with -A, when also feeding in non-brain-extracted T2 (includes registrations)

Miscellaneous options:
  -v          verbose (switch on diagnostic messages)
  -h          display this help, then exits
  -d          debug (don't delete temporary intermediate images)

";

/// The line of the script that calls `imglob` without `${FSLDIR}/bin` (bash names it when the
/// program is not found).
const IMGLOB_LINE: u32 = 329;

/// The line bash names in its report of a program that crashed inside the variations' `if`
/// block: the block's end (`fi`), where bash's parser stood when it ran the block.
const VARIATIONS_FI: u32 = 399;

/// The standard images the variations read (relative to `${FSLDIR}/data`).
const STD_T1: &str = "standard/MNI152_T1_2mm";
const STD_BRAIN_MASK_DIL: &str = "standard/MNI152_T1_2mm_brain_mask_dil";
const STD_EYE_MASK: &str = "standard/MNI152_T1_2mm_eye_mask";

/// A program the script runs (the replica's, in this process).
type Program =
    fn(&[String], &Settings, UpstreamDefects, usize, &mut dyn Write, &mut dyn Write) -> u8;

/// `standard_space_roi`, `flirt`, `fslmaths`, `fslstats`, `fast` as the script runs them.
fn ssroi(
    a: &[String],
    s: &Settings,
    m: UpstreamDefects,
    t: usize,
    o: &mut dyn Write,
    e: &mut dyn Write,
) -> u8 {
    larmorx_nc_flirt::standard_space_roi(a, s, m, t, o, e)
}

/// Runs `f` in this process: its exit status, or, where FSL's process aborts, what remains to
/// print on standard error before bash's report (an exception nothing in the program catches:
/// what FSL printed from the throw on; a program that printed FSL's text and its own abort
/// line, as FLIRT's do: nothing more). A script (`standard_space_roi`) reports its own
/// programs' aborts and goes on: its lines are not watched.
fn call(
    run: &Run,
    f: Program,
    args: &[String],
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, String> {
    let (fsl, mode, threads) = (run.fsl, run.mode, run.threads);
    let script = std::ptr::fn_addr_eq(f, ssroi as Program);
    let mut watch = exception::AbortWatch::new(err);
    let r = exception::catch(|| f(args, fsl, mode, threads, out, &mut watch));
    let aborted = watch.aborted() && !script;
    let _ = out.flush();
    match r {
        Ok(_) if aborted => Err(String::new()),
        Ok(c) => Ok(c),
        Err(e) => Err(e.stderr_text()),
    }
}

/// How the script stops.
enum Stop {
    /// `exit <code>` (the usage, an invalid input, a refusal).
    Exit(u8),
    /// A command failed: the `ERR` trap prints `$0 failed during command:<args>` and exits 1.
    Failed,
}

struct Script<'a, 'w> {
    run: &'a Run<'a>,
    out: &'w mut dyn Write,
    err: &'w mut dyn Write,
    notices: Notices,
    /// The script's arguments, for the ERR trap's message.
    args: Vec<String>,
    /// Files this run created (the correction of the cleanup's glob, FSL-46).
    created: Vec<String>,
    /// The files named `<output>_tmp...` before the run, with their modification times: a file
    /// not among them, or changed since, is this run's (FSL-46; the programs' many temporary
    /// files: FAST's, standard_space_roi's with `-d`).
    before: Vec<(String, Option<SystemTime>)>,
}

/// `bet`'s main (`args[0]` is the program name).
pub fn main(args: &[String], run: &Run, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let (code, notices) = run_script(args, run, out, err);
    crate::bet2::print_notices(&notices, err);
    code
}

/// The script: its exit status and its notices (printed by the caller; `-B` runs the script
/// again inside itself and keeps one set).
fn run_script(
    args: &[String],
    run: &Run,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> (u8, Notices) {
    let mut s = Script {
        run,
        out,
        err,
        notices: Notices::new(run.mode),
        args: args[1..].to_vec(),
        created: Vec::new(),
        before: Vec::new(),
    };
    let code = match s.body() {
        Ok(()) => 0,
        Err(Stop::Exit(c)) => c,
        Err(Stop::Failed) => {
            // trap "script_error $*" ERR: the arguments as words, joined by blanks
            let words: Vec<String> = s.args.iter().flat_map(|a| split_words(a)).collect();
            let _ = writeln!(s.out, "bet failed during command:{}", words.join(" "));
            1
        }
    };
    let notices = std::mem::replace(&mut s.notices, Notices::new(run.mode));
    (code, notices)
}

/// A file's modification time (`None` when it cannot be read).
fn modified(f: &str) -> Option<SystemTime> {
    std::fs::symlink_metadata(f).and_then(|m| m.modified()).ok()
}

/// The words of a literal part of a command.
fn lit(s: &str) -> Vec<String> {
    split_words(s)
}

/// A command's words, part after part.
fn cat<const N: usize>(parts: [Vec<String>; N]) -> Vec<String> {
    parts.into_iter().flatten().collect()
}

/// Bash's word splitting of an unquoted expansion (default IFS: blanks, tabs, newlines).
fn split_words(s: &str) -> Vec<String> {
    s.split([' ', '\t', '\n'])
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

impl Script<'_, '_> {
    fn usage(&mut self) -> Stop {
        let _ = self.out.write_all(USAGE.as_bytes());
        Stop::Exit(1)
    }

    fn reproduce(&self) -> bool {
        self.notices.reproduce()
    }

    /// The words an unquoted `$x` gives: split on blanks in `reproduce` mode (FSL-44), the
    /// value itself by default.
    fn words(&mut self, value: &str, what: &str) -> Vec<String> {
        let split = split_words(value);
        let differs = split.len() != 1 || split[0] != value;
        if differs {
            if self.reproduce() {
                self.notices.add(
                    &defects::FSL_44,
                    format!(
                        "{what} '{value}' split into {} word(s), as the script does",
                        split.len()
                    ),
                );
                return split;
            }
            self.notices.add(
                &defects::FSL_44,
                format!("{what} '{value}' used as one name (the script splits it into words)"),
            );
        }
        if value.is_empty() {
            return if self.reproduce() {
                Vec::new()
            } else {
                vec![String::new()]
            };
        }
        vec![value.to_owned()]
    }

    fn fsl(&self) -> &Settings {
        self.run.fsl
    }

    /// Runs `bet2` with these words; a failure stops the script through its ERR trap.
    fn bet2(&mut self, words: &[String]) -> Result<(), Stop> {
        let mut argv = vec!["bet2".to_owned()];
        argv.extend(words.iter().cloned());
        // Where FSL's bet2 crashes on what it leaves undefined, the script's ERR trap stops it
        // with its failure message: that, FSL's defined outcome, is what `reproduce` gives here
        // (bet2 alone fills the undefined part instead: D13's failure cases).
        let run = Run {
            fsl: self.run.fsl,
            mode: self.run.mode,
            threads: self.run.threads,
            fill: false,
        };
        // bet2's notices join this run's (one per defect, printed at the end)
        let code = bet2::main_with(&argv, &run, self.out, self.err, &mut self.notices);
        if code != 0 {
            return Err(Stop::Failed);
        }
        Ok(())
    }

    fn tool<T>(&mut self, r: Result<T, ToolFailure>) -> Result<T, Stop> {
        match r {
            Ok(v) => Ok(v),
            Err(f) => {
                let _ = self.err.write_all(f.err.as_bytes());
                if f.code != 0 {
                    Err(Stop::Failed)
                } else {
                    Err(Stop::Exit(0))
                }
            }
        }
    }

    /// Records an output of this run (for the corrected cleanup).
    fn created(&mut self, base: &str) {
        self.created.push(base.to_owned());
    }

    fn body(&mut self) -> Result<(), Stop> {
        let a = self.args.clone();
        // [ "$2" = "" ] && Usage
        if a.len() < 2 || a[1].is_empty() {
            return Err(self.usage());
        }
        let in_words = self.words(&a[0], "the input");
        let out_words = self.words(&a[1], "the output");
        // IN=`remove_ext $1`, OUT=`remove_ext $2`: remove_ext prints its usage and fails
        // without arguments
        if in_words.is_empty() || out_words.is_empty() {
            // remove_ext without arguments prints its usage (into the variable) and fails:
            // the ERR trap stops the script
            return Err(Stop::Failed);
        }
        let in_name = in_words
            .iter()
            .map(|w| avw::remove_ext(w))
            .collect::<Vec<_>>()
            .join(" ");
        let out_name = out_words
            .iter()
            .map(|w| avw::remove_ext(w))
            .collect::<Vec<_>>()
            .join(" ");
        // [ `imtest $IN` = 0 ]: imtest is false unless it gets exactly one word
        let in_test = if self.reproduce() {
            split_words(&in_name)
        } else {
            vec![in_name.clone()]
        };
        if in_test.len() != 1 || !avw::imtest(&in_test[0]) {
            let _ = writeln!(self.out, "\nError: input image {in_name} not valid\n");
            return Err(Stop::Exit(1));
        }

        let mut debug = false;
        let mut verbose = false;
        let mut variation = 0;
        let mut skull = false;
        let mut bet2opts = String::new();
        let mut in2 = String::new();
        let mut i = 2;
        while i < a.len() {
            let o = a[i].as_str();
            if o.is_empty() || (self.reproduce() && split_words(o).len() > 1) {
                // while [ _$1 != _ ]: an empty argument ends the options (and one with a blank
                // makes the test fail: bash prints "[: too many arguments")
                break;
            }
            let arg = |k: usize| a.get(i + k).map_or("", String::as_str);
            match o {
                "-o" | "-m" | "-s" | "-n" | "-t" | "-e" => {
                    bet2opts.push(' ');
                    bet2opts.push_str(o);
                    if o == "-s" {
                        skull = true;
                    }
                    i += 1;
                }
                "-f" | "-g" | "-r" => {
                    if arg(1).is_empty() {
                        return Err(self.usage());
                    }
                    bet2opts.push_str(&format!(" {o} {}", arg(1)));
                    i += 2;
                }
                "-c" => {
                    if arg(3).is_empty() {
                        return Err(self.usage());
                    }
                    bet2opts.push_str(&format!(" {o} {} {} {}", arg(1), arg(2), arg(3)));
                    i += 4;
                }
                "-R" => {
                    variation = 1;
                    i += 1;
                }
                "-S" => {
                    variation = 2;
                    i += 1;
                }
                "-Z" => {
                    variation = 3;
                    i += 1;
                }
                "-F" => {
                    variation = 4;
                    i += 1;
                }
                "-A" => {
                    variation = 5;
                    i += 1;
                }
                "-A2" => {
                    if arg(1).is_empty() {
                        return Err(self.usage());
                    }
                    variation = 6;
                    in2 = arg(1).to_owned();
                    i += 2;
                }
                "-B" => {
                    variation = 7;
                    i += 1;
                }
                "-v" => {
                    verbose = true;
                    bet2opts.push_str(" -v");
                    i += 1;
                }
                "-d" => {
                    debug = true;
                    i += 1;
                }
                _ => return Err(self.usage()),
            }
        }
        if verbose {
            let _ = writeln!(self.out, "IN={}", split_words(&in_name).join(" "));
            let _ = writeln!(self.out, "OUT={}", split_words(&out_name).join(" "));
            let _ = writeln!(self.out, "bet2opts={bet2opts}");
            let _ = writeln!(self.out, "verbose={}", u8::from(verbose));
            let _ = writeln!(self.out, "debug={}", u8::from(debug));
            let _ = writeln!(self.out, "variation={variation}");
        }
        let opts = split_words(&bet2opts);
        let ins = if self.reproduce() {
            split_words(&in_name)
        } else {
            vec![in_name.clone()]
        };
        let outs = if self.reproduce() {
            split_words(&out_name)
        } else {
            vec![out_name.clone()]
        };
        let pair = |extra: &[String]| -> Vec<String> {
            let mut v: Vec<String> = ins.clone();
            v.extend(outs.iter().cloned());
            v.extend(extra.iter().cloned());
            v
        };
        // the standard images the variation reads, before anything runs (FSL's script would
        // fail at the first program that reads a missing one)
        let needed: &[&str] = match variation {
            2 => &[STD_T1, STD_BRAIN_MASK_DIL, STD_EYE_MASK],
            5 | 6 => &[STD_T1],
            7 => &[STD_T1, STD_BRAIN_MASK_DIL],
            _ => &[],
        };
        for rel in needed {
            self.data(rel)?;
        }
        // the files named ${OUT}_tmp... before the run (the cleanup's correction, FSL-46)
        self.before = avw::shell_glob_prefix(&format!("{out_name}_tmp"))
            .into_iter()
            .map(|f| {
                let m = modified(&f);
                (f, m)
            })
            .collect();
        match variation {
            0 => self.bet2(&pair(&opts))?,
            1 => self.robust(&ins, &outs, &bet2opts)?,
            2 => self.eyes(&in_name, &out_name, &bet2opts)?,
            3 => return self.small_fov(&in_name, &out_name, &opts, debug),
            4 => self.fmri(&in_name, &out_name, &opts, debug)?,
            5 => self.surfaces(&in_name, &out_name, &opts, None)?,
            6 => self.surfaces(&in_name, &out_name, &opts, Some(&in2))?,
            7 => self.bias(&in_name, &out_name, &bet2opts, skull)?,
            _ => {}
        }
        self.cleanup(&out_name, debug);
        Ok(())
    }

    /// The standard image `rel`'s name as FSL's programs get it, or the refusal (exit 1) naming
    /// it and every place looked.
    fn data(&mut self, rel: &str) -> Result<String, Stop> {
        match fsldata::find(rel, self.fsl()) {
            Ok(f) => Ok(f.name),
            Err(e) => {
                let _ = writeln!(self.err, "larmorx-nc: error: bet: {e}");
                Err(Stop::Exit(1))
            }
        }
    }

    /// `$v` as a command gets it, without a notice (the arguments' own were named when read):
    /// its words in `reproduce` (FSL-44), the value itself by default.
    fn wq(&self, v: &str) -> Vec<String> {
        if self.reproduce() {
            split_words(v)
        } else {
            vec![v.to_owned()]
        }
    }

    /// `${OUT}_<suffix>` as a command gets it.
    fn tmp(&self, out: &str, suffix: &str) -> Vec<String> {
        self.wq(&format!("{out}_{suffix}"))
    }

    /// Runs one of the script's programs (`source`: its line in the script, for bash's report
    /// of a crash); a failure stops the script through its `ERR` trap.
    fn program(&mut self, source: &str, f: Program, args: Vec<String>) -> Result<(), Stop> {
        let r = call(self.run, f, &args, &mut *self.out, &mut *self.err);
        self.status(r, source)
    }

    /// A program's end as bash sees it: its status, or its crash (what FSL printed, then bash's
    /// report on the script's standard error); a failure stops the script (the `ERR` trap).
    fn status(&mut self, r: Result<u8, String>, source: &str) -> Result<(), Stop> {
        let code = match r {
            Ok(c) => c,
            Err(text) => {
                let _ = self.err.write_all(text.as_bytes());
                self.aborted(source);
                134
            }
        };
        if code != 0 {
            return Err(Stop::Failed);
        }
        Ok(())
    }

    /// bash's report of a program that aborted (its process ID masked by the suites).
    fn aborted(&mut self, source: &str) {
        let _ = writeln!(
            self.err,
            "bet: line {VARIATIONS_FI}: {:>5} Aborted                 {source}",
            std::process::id()
        );
    }

    /// `` V=`<program> args` ``: what the program printed (bash's command substitution, the
    /// trailing newlines removed); a failure stops the script.
    fn captured(&mut self, f: Program, args: Vec<String>) -> Result<String, Stop> {
        let mut buf = Vec::new();
        let r = call(self.run, f, &args, &mut buf, &mut *self.err);
        match r {
            Ok(0) => {}
            Ok(_) => return Err(Stop::Failed),
            Err(text) => {
                let _ = self.err.write_all(text.as_bytes());
                return Err(Stop::Failed);
            }
        }
        let s = String::from_utf8_lossy(&buf).into_owned();
        Ok(s.trim_end_matches('\n').to_owned())
    }

    /// `rm -f ${OUT}_tmp*` (unless `-d`): by default only this run's temporary files; the
    /// script's glob also removes any other file named so (FSL-46).
    fn cleanup(&mut self, out_name: &str, debug: bool) {
        if debug {
            return;
        }
        let prefix = format!("{out_name}_tmp");
        let globbed: Vec<String> = avw::shell_glob_prefix(&prefix)
            .into_iter()
            .filter(|f| std::path::Path::new(f).is_file() || std::path::Path::new(f).is_symlink())
            .collect();
        let ours: Vec<String> = globbed
            .iter()
            .filter(|f| {
                self.created
                    .iter()
                    .any(|c| c.starts_with(&prefix) && avw::remove_ext(f) == *c)
                    || !self
                        .before
                        .iter()
                        .any(|(b, m)| b == *f && *m == modified(f))
            })
            .cloned()
            .collect();
        let others: Vec<&String> = globbed.iter().filter(|f| !ours.contains(f)).collect();
        if !others.is_empty() {
            let names: Vec<&str> = others.iter().map(|s| s.as_str()).collect();
            if self.reproduce() {
                self.notices.add(
                    &defects::FSL_46,
                    format!(
                        "removed files this run did not create, as the script does: {}",
                        names.join(", ")
                    ),
                );
            } else {
                self.notices.add(
                    &defects::FSL_46,
                    format!(
                        "kept files this run did not create (the script removes them): {}",
                        names.join(", ")
                    ),
                );
            }
        }
        let doomed = if self.reproduce() {
            globbed.clone()
        } else {
            ours
        };
        for f in doomed {
            let _ = std::fs::remove_file(&f);
        }
    }

    /// `` bet2opts=`echo $bet2opts | sed 's/<pat>//g'` `` (in `-R` and in `-B` with `-s`; `who`
    /// the variation and `whom` what gets the options, for the notice): bash's echo takes the
    /// leading words made of `-n`, `-e` and `-E` as its own options (FSL-63), and the command
    /// substitution drops the trailing newline. By default the options given, less `pat`; in
    /// `reproduce` what the script's echo and sed leave. FSL-63's notice, where the two differ,
    /// is returned: the caller names it once a run given those options has finished (a run that
    /// fails first shows no difference).
    fn echo_sed(
        &mut self,
        bet2opts: &str,
        pat: &str,
        who: &str,
        whom: &str,
    ) -> (Vec<String>, Option<String>) {
        let words = split_words(bet2opts);
        let given = split_words(&words.join(" ").replace(pat, ""));
        let (printed, taken) = echo::run(&words);
        let theirs = split_words(printed.replace(pat, "").trim_end_matches('\n'));
        if theirs == given {
            (given, None)
        } else if self.reproduce() {
            let detail = format!(
                "{who} passes bet2's options through bash's echo, which takes '{}' as its own: \
                 {whom} gets '{}', as in FSL",
                taken.join(" "),
                theirs.join(" ")
            );
            (theirs, Some(detail))
        } else {
            let detail = format!(
                "{who} passes bet2's options through bash's echo, which takes '{}' as its own \
                 (FSL's {whom} gets '{}'): {whom} gets '{}', the options given",
                taken.join(" "),
                theirs.join(" "),
                given.join(" ")
            );
            (given, Some(detail))
        }
    }

    /// Names FSL-63 (from [`Self::echo_sed`]) once the run given the options has finished.
    fn echo_named(&mut self, detail: &mut Option<String>) {
        if let Some(d) = detail.take() {
            self.notices.add(&defects::FSL_63, d);
        }
    }

    /// Runs `betsurf` with these words (its notices joining this run's); a failure stops the
    /// script.
    fn betsurf(&mut self, words: Vec<String>) -> Result<(), Stop> {
        let mut argv = vec!["betsurf".to_owned()];
        argv.extend(words);
        let run = Run {
            fsl: self.run.fsl,
            mode: self.run.mode,
            threads: self.run.threads,
            fill: false,
        };
        let code = crate::betsurf::main_with(&argv, &run, self.out, self.err, &mut self.notices);
        if code != 0 {
            return Err(Stop::Failed);
        }
        Ok(())
    }

    /// `-S`: the SIENA eye and optic nerve cleanup.
    fn eyes(&mut self, inn: &str, out: &str, bet2opts: &str) -> Result<(), Stop> {
        let opts = split_words(bet2opts);
        let ins = self.wq(inn);
        let outs = self.wq(out);
        let t = |s: &Self, suffix: &str| s.tmp(out, suffix);
        // get the raw brain and skull images
        let w = cat([ins.clone(), t(self, "tmp_betraw"), lit("-s"), opts.clone()]);
        self.bet2(&w)?;
        // run betpremask (keep intermediate files around using -d) and feed it into bet
        let w = cat([ins.clone(), t(self, "tmp_premask"), lit("-b -d")]);
        self.program(
            "${FSLDIR}/bin/standard_space_roi $IN ${OUT}_tmp_premask -b -d",
            ssroi,
            w,
        )?;
        let w = cat([
            t(self, "tmp_premask"),
            t(self, "tmp_premask_bet"),
            lit("-n -m -f 0.4"),
            opts.clone(),
        ]);
        self.bet2(&w)?;
        // bring eye mask from standard space into native and dilate, mostly in 2D and then
        // invert
        let eye = self.data(STD_EYE_MASK)?;
        let w = cat([
            lit("-ref"),
            ins.clone(),
            lit("-in"),
            vec![eye],
            lit("-applyxfm -init"),
            t(self, "tmp_premask_tmp_to_std_inv.mat"),
            lit("-datatype float -out"),
            t(self, "tmp_eye_mask"),
        ]);
        self.program(
            "${FSLDIR}/bin/flirt -ref $IN -in ${FSLDIR}/data/standard/MNI152_T1_2mm_eye_mask -applyxfm -init ${OUT}_tmp_premask_tmp_to_std_inv.mat -datatype float -out ${OUT}_tmp_eye_mask",
            larmorx_nc_flirt::flirt,
            w,
        )?;
        let w = cat([
            t(self, "tmp_eye_mask"),
            lit("-thr 0.5 -bin -dilF -kernel 2D -dilF -dilF -dilF -dilF -dilF -dilF -dilF"),
            t(self, "tmp_eyes7dil"),
            lit("-odt short"),
        ]);
        self.program(
            "${FSLDIR}/bin/fslmaths ${OUT}_tmp_eye_mask -thr 0.5 -bin -dilF -kernel 2D -dilF -dilF -dilF -dilF -dilF -dilF -dilF ${OUT}_tmp_eyes7dil -odt short",
            larmorx_nc_avwutils::fslmaths,
            w,
        )?;
        let w = cat([
            t(self, "tmp_eyes7dil"),
            lit("-mul -1 -add 1"),
            t(self, "tmp_eyes7dil_inv"),
        ]);
        self.program(
            "${FSLDIR}/bin/fslmaths ${OUT}_tmp_eyes7dil -mul -1 -add 1 ${OUT}_tmp_eyes7dil_inv",
            larmorx_nc_avwutils::fslmaths,
            w,
        )?;
        // mask original brain mask with inverted-eye-mask, ADD betpremask_bet_mask and X by
        // original brain
        let w = cat([
            t(self, "tmp_betraw"),
            lit("-bin -mas"),
            t(self, "tmp_eyes7dil_inv"),
            lit("-add"),
            t(self, "tmp_premask_bet_mask"),
            lit("-bin -mul"),
            t(self, "tmp_betraw"),
            t(self, "tmp_prebrain"),
        ]);
        self.program(
            "${FSLDIR}/bin/fslmaths ${OUT}_tmp_betraw -bin -mas ${OUT}_tmp_eyes7dil_inv -add ${OUT}_tmp_premask_bet_mask -bin -mul ${OUT}_tmp_betraw ${OUT}_tmp_prebrain",
            larmorx_nc_avwutils::fslmaths,
            w,
        )?;
        // find upper threshold used to delete eyes
        let mean = self.captured(
            larmorx_nc_avwutils::fslstats,
            cat([t(self, "tmp_prebrain"), lit("-M")]),
        )?;
        let sd = self.captured(
            larmorx_nc_avwutils::fslstats,
            cat([t(self, "tmp_prebrain"), lit("-S")]),
        )?;
        let thr = split_words(&self.eye_threshold(&mean, &sd)?);
        // apply threshold and a little morphology (twice) to cleanup further and produce final
        // brain image
        let w = cat([
            t(self, "tmp_prebrain"),
            lit("-uthr"),
            thr.clone(),
            lit("-bin -ero -dilF -dilF -ero -mul"),
            t(self, "tmp_prebrain"),
            lit("-uthr"),
            thr,
            lit("-bin -kernel 2D -ero -dilF -mul"),
            t(self, "tmp_betraw"),
            outs.clone(),
        ]);
        self.program(
            "${FSLDIR}/bin/fslmaths ${OUT}_tmp_prebrain -uthr $thr -bin -ero -dilF -dilF -ero -mul ${OUT}_tmp_prebrain -uthr $thr -bin -kernel 2D -ero -dilF -mul ${OUT}_tmp_betraw $OUT",
            larmorx_nc_avwutils::fslmaths,
            w,
        )?;
        let w = cat([outs.clone(), lit("-bin"), t(self, "mask")]);
        self.program(
            "${FSLDIR}/bin/fslmaths $OUT -bin ${OUT}_mask",
            larmorx_nc_avwutils::fslmaths,
            w,
        )?;
        // remove eyes from original skull to produce final skull image
        let w = cat([
            t(self, "tmp_eyes7dil"),
            lit("-ero -kernel 2D -ero -mul -1 -add 1"),
            t(self, "tmp_eyes5dil_inv"),
        ]);
        self.program(
            "${FSLDIR}/bin/fslmaths ${OUT}_tmp_eyes7dil -ero -kernel 2D -ero -mul -1 -add 1 ${OUT}_tmp_eyes5dil_inv",
            larmorx_nc_avwutils::fslmaths,
            w,
        )?;
        let w = cat([
            t(self, "tmp_betraw_skull"),
            lit("-mas"),
            t(self, "tmp_eyes5dil_inv"),
            t(self, "skull"),
        ]);
        self.program(
            "${FSLDIR}/bin/fslmaths ${OUT}_tmp_betraw_skull -mas ${OUT}_tmp_eyes5dil_inv ${OUT}_skull",
            larmorx_nc_avwutils::fslmaths,
            w,
        )
    }

    /// `` thr=`echo "10 k 3.5 $SD * $Mean + p" | dc` ``: 3.5 standard deviations above the
    /// brain's mean, in GNU dc's decimal arithmetic. dc reads what fslstats printed as dc's
    /// own text (FSL-216): an exponent (`1.23457e+06`), a minus sign, `nan` or `inf` become
    /// commands. By default the numbers as printed are read as numbers (an exponent or a minus
    /// sign written in dc's notation), and a mean or deviation that is not a number is refused;
    /// `reproduce` gives what FSL's dc prints (its messages on standard error).
    fn eye_threshold(&mut self, mean: &str, sd: &str) -> Result<String, Stop> {
        let mut msgs = Vec::new();
        let theirs = dc::eval(&format!("10 k 3.5 {sd} * {mean} + p"), &mut msgs);
        let theirs = theirs.trim_end_matches('\n').to_owned();
        let (m, s) = (dc::decimal(mean.trim()), dc::decimal(sd.trim()));
        let (Some(m), Some(s)) = (m, s) else {
            if self.reproduce() {
                self.notices.add(
                    &defects::FSL_216,
                    format!(
                        "-S's eye threshold from a mean of '{}' and a standard deviation of \
                         '{}' (the brain holds values that are not numbers): '{theirs}', what \
                         FSL's dc prints",
                        mean.trim(),
                        sd.trim()
                    ),
                );
                let _ = self.err.write_all(&msgs);
                return Ok(theirs);
            }
            self.notices.add(
                &defects::FSL_216,
                format!(
                    "refused: -S's eye threshold needs the brain's mean and standard deviation, \
                     and fslstats printed '{}' and '{}' (the brain holds values that are not \
                     numbers; FSL's dc reads them as its commands and gives '{theirs}')",
                    mean.trim(),
                    sd.trim()
                ),
            );
            let _ = writeln!(
                self.err,
                "larmorx-nc: error: bet: -S: the brain's mean ({}) or standard deviation ({}) \
                 is not a number, so its eye threshold is undefined (FSL-216)",
                mean.trim(),
                sd.trim()
            );
            return Err(Stop::Exit(1));
        };
        let ours = dc::eval(&format!("10 k 3.5 {s} * {m} + p"), &mut Vec::new());
        let ours = ours.trim_end_matches('\n').to_owned();
        if ours == theirs && msgs.is_empty() {
            return Ok(theirs);
        }
        if self.reproduce() {
            self.notices.add(
                &defects::FSL_216,
                format!(
                    "-S's eye threshold from fslstats' '{}' and '{}': '{theirs}', as FSL's dc \
                     reads them (3.5 standard deviations above the mean: {ours})",
                    mean.trim(),
                    sd.trim()
                ),
            );
            let _ = self.err.write_all(&msgs);
            return Ok(theirs);
        }
        self.notices.add(
            &defects::FSL_216,
            format!(
                "-S's eye threshold from fslstats' '{}' and '{}': {ours}, 3.5 standard \
                 deviations above the mean (FSL's dc reads the numbers as commands: '{theirs}')",
                mean.trim(),
                sd.trim()
            ),
        );
        Ok(ours)
    }

    /// `-B`: the SIENA bias field and neck cleanup.
    fn bias(&mut self, inn: &str, out: &str, bet2opts: &str, skull: bool) -> Result<(), Stop> {
        let ins = self.wq(inn);
        let outs = self.wq(out);
        let t = |s: &Self, suffix: &str| s.tmp(out, suffix);
        // run betpremask
        let w = cat([ins.clone(), t(self, "tmp_premask"), lit("-b -d")]);
        self.program(
            "${FSLDIR}/bin/standard_space_roi $IN ${OUT}_tmp_premask -b -d",
            ssroi,
            w,
        )?;
        // run FAST to get bias field: fast -b ${OUT}_tmp_premask >/dev/null 2>&1 (larmorx's
        // own lines, its notices and refusals, still shown)
        let w = cat([lit("-b"), t(self, "tmp_premask")]);
        let mut discarded = Vec::new();
        let r = call(
            self.run,
            larmorx_nc_fast::fast,
            &w,
            &mut std::io::sink(),
            &mut discarded,
        );
        let ours: String = String::from_utf8_lossy(&discarded)
            .lines()
            .filter(|l| l.starts_with("larmorx-nc: "))
            .map(|l| format!("{l}\n"))
            .collect();
        let _ = self.err.write_all(ours.as_bytes());
        let r = match r {
            Err(_) => {
                // what FAST printed went to /dev/null; bash reports the crash itself
                self.aborted("${FSLDIR}/bin/fast -b ${OUT}_tmp_premask > /dev/null 2>&1");
                Ok(134)
            }
            ok => ok,
        };
        self.status(r, "")?;
        // apply bias correction to original
        let w = cat([
            ins.clone(),
            lit("-div"),
            t(self, "tmp_premask_bias"),
            t(self, "tmp_unbiased"),
        ]);
        self.program(
            "${FSLDIR}/bin/fslmaths $IN -div ${OUT}_tmp_premask_bias ${OUT}_tmp_unbiased",
            larmorx_nc_avwutils::fslmaths,
            w,
        )?;
        let mut opts = split_words(bet2opts);
        let mut echoed = None;
        // get skull if asked for
        if skull {
            let w = cat([
                t(self, "tmp_unbiased"),
                t(self, "tmp_unbiased_forskull_reduced"),
                lit("-maskFOV -roiNONE -d"),
            ]);
            self.program(
                "${FSLDIR}/bin/standard_space_roi ${OUT}_tmp_unbiased ${OUT}_tmp_unbiased_forskull_reduced -maskFOV -roiNONE -d",
                ssroi,
                w,
            )?;
            let w = cat([
                t(self, "tmp_unbiased_forskull_reduced"),
                t(self, "tmp_unbiased_forskull"),
                opts.clone(),
            ]);
            self.bet2(&w)?;
            // bet2opts=`echo $bet2opts | sed 's/-s//g'`
            (opts, echoed) = self.echo_sed(bet2opts, "-s", "-B with -s", "the robust bet");
            let w = cat([t(self, "tmp_unbiased_forskull_skull"), t(self, "skull")]);
            self.immv(w)?;
        }
        // re-run betpremask
        let w = cat([
            t(self, "tmp_unbiased"),
            t(self, "tmp_unbiased_premask"),
            lit("-b -d"),
        ]);
        self.program(
            "${FSLDIR}/bin/standard_space_roi ${OUT}_tmp_unbiased ${OUT}_tmp_unbiased_premask -b -d",
            ssroi,
            w,
        )?;
        // re-run in robust mode: ${FSLDIR}/bin/bet ${OUT}_tmp_unbiased_premask $OUT -m
        // $bet2opts -R -d, this script again (its own ERR trap's message, then this one's)
        let mut argv = vec!["bet".to_owned()];
        argv.extend(cat([
            t(self, "tmp_unbiased_premask"),
            outs,
            lit("-m"),
            opts,
            lit("-R -d"),
        ]));
        let (code, notices) = run_script(&argv, self.run, &mut *self.out, &mut *self.err);
        self.notices.extend(notices.into_vec());
        if code != 0 {
            return Err(Stop::Failed);
        }
        self.echo_named(&mut echoed);
        Ok(())
    }

    /// `${FSLDIR}/bin/immv <words>` (fslpy's): the last word the destination, a directory when
    /// there are several sources; its usage or failure on standard output, status 1.
    fn immv(&mut self, mut words: Vec<String>) -> Result<(), Stop> {
        if words.len() < 2
            || (words.len() > 2 && !std::path::Path::new(&words[words.len() - 1]).is_dir())
        {
            let usage = fslpy::IMCP_USAGE.replace("imcp", "immv").replace(
                "Copy images from <file1> to <file2>, or copy all",
                "Moves images from <file1> to <file2>, or move all",
            );
            let _ = writeln!(self.out, "{usage}");
            return Err(Stop::Failed);
        }
        let dest = words.pop().unwrap_or_default();
        for src in &words {
            match fslpy::copy(src, &dest, self.fsl(), true) {
                Ok(()) => {}
                Err(f) if f.unsupported => {
                    let _ = writeln!(self.err, "larmorx-nc: error: bet: {}", f.text);
                    return Err(Stop::Exit(1));
                }
                Err(f) => {
                    if let Some(w) = &f.warning {
                        let _ = writeln!(self.err, "{w}");
                    }
                    let _ = writeln!(self.out, "{}", f.text);
                    return Err(Stop::Failed);
                }
            }
        }
        Ok(())
    }

    /// `-A` (`t2` absent) and `-A2 <T2>`: bet2's mesh, then betsurf's skull and scalp surfaces.
    fn surfaces(
        &mut self,
        inn: &str,
        out: &str,
        opts: &[String],
        t2: Option<&str>,
    ) -> Result<(), Stop> {
        let ins = self.wq(inn);
        let outs = self.wq(out);
        let t = |s: &Self, suffix: &str| s.tmp(out, suffix);
        let std = self.data(STD_T1)?;
        if let Some(t2) = t2 {
            // register T2 to T1
            let t2w = self.words(t2, "the T2");
            let w = cat([
                lit("-in"),
                t2w,
                lit("-ref"),
                ins.clone(),
                lit("-out"),
                t(self, "tmp_T2_to_T1"),
                lit("-omat"),
                t(self, "tmp_T2_to_T1.mat"),
                lit("-interp sinc"),
            ]);
            self.program(
                "${FSLDIR}/bin/flirt -in $IN2 -ref $IN -out ${OUT}_tmp_T2_to_T1 -omat ${OUT}_tmp_T2_to_T1.mat -interp sinc",
                larmorx_nc_flirt::flirt,
                w,
            )?;
        }
        // register T1 to standard space
        let w = cat([
            lit("-ref"),
            vec![std],
            lit("-in"),
            ins.clone(),
            lit("-omat"),
            t(self, "tmp_T1_to_std.mat"),
        ]);
        self.program(
            "${FSLDIR}/bin/flirt -ref ${FSLDIR}/data/standard/MNI152_T1_2mm -in $IN -omat ${OUT}_tmp_T1_to_std.mat",
            larmorx_nc_flirt::flirt,
            w,
        )?;
        // run brain extraction and save brain mesh
        let w = cat([ins.clone(), outs.clone(), lit("-e"), opts.to_vec()]);
        self.bet2(&w)?;
        // find other surfaces
        let w = if t2.is_some() {
            cat([
                lit("-o -m -s"),
                ins,
                t(self, "tmp_T2_to_T1"),
                t(self, "mesh.vtk"),
                t(self, "tmp_T1_to_std.mat"),
                outs,
            ])
        } else {
            cat([
                lit("--t1only -o -m -s"),
                ins,
                t(self, "mesh.vtk"),
                t(self, "tmp_T1_to_std.mat"),
                outs,
            ])
        };
        self.betsurf(w)
    }

    /// `-R`.
    fn robust(&mut self, ins: &[String], outs: &[String], bet2opts: &str) -> Result<(), Stop> {
        // bet2opts=`echo $bet2opts | sed 's/-n//g'`
        let (opts, mut echoed) = self.echo_sed(bet2opts, "-n", "-R", "bet2");
        let out = outs.join(" ");
        // get_dims $IN: fslval runs (their values are unused by -R)
        let mut i: i64 = 1;
        let (mut x, mut y, mut z) = (String::new(), String::new(), String::new());
        loop {
            if i <= 0 {
                break;
            }
            if i == 1 {
                let mut w = ins.to_vec();
                w.extend(outs.iter().cloned());
                w.extend(opts.iter().cloned());
                self.bet2(&w)?;
                self.echo_named(&mut echoed);
                let c = self.cog(&out)?;
                (x, y, z) = c;
            } else {
                let mut w = ins.to_vec();
                w.extend(outs.iter().cloned());
                w.extend(opts.iter().cloned());
                w.extend(["-c".to_owned(), x.clone(), y.clone(), z.clone()]);
                self.bet2(&w)?;
                let (ox, oy, oz) = (x.clone(), y.clone(), z.clone());
                let c = self.cog(&out)?;
                (x, y, z) = c;
                let expr = format!("0 k {ox} {x} - 2 ^ {oy} {y} - 2 ^ + {oz} {z} - 2 ^ + 1 / p");
                let diffsq = dc::eval(&expr, self.err);
                let d: i64 = diffsq.trim().parse().unwrap_or(0);
                if d < 1 || i > 10 {
                    i = -10;
                }
            }
            i += 1;
        }
        Ok(())
    }

    /// `find_centre_in_voxels`: `fslstats $1 -l 0.001 -C`, its three first words.
    fn cog(&mut self, name: &str) -> Result<(String, String, String), Stop> {
        let r = avw::fslstats_cog(name, "0.001", self.fsl());
        let (line, warnings) = self.tool(r)?;
        let _ = self.err.write_all(warnings.as_bytes());
        let w = split_words(&line);
        let g = |k: usize| w.get(k).cloned().unwrap_or_default();
        Ok((g(0), g(1), g(2)))
    }

    /// FSL-217 after fslroi or fslmerge wrote an int8 image: named where the result differs
    /// from FSL's, when a value was negative (bet2 runs on other values) or the corrected int8
    /// file is kept (`-d`); bet2's outputs are float either way.
    fn int8_written(&mut self, written: Option<avw::Int8Output>, program: &str, debug: bool) {
        if let Some(w) = written
            && (w.negative > 0 || (w.kept && debug))
        {
            self.notices.add(&defects::FSL_217, w.detail(program));
        }
    }

    /// `-F`.
    fn fmri(
        &mut self,
        in_name: &str,
        out_name: &str,
        opts: &[String],
        debug: bool,
    ) -> Result<(), Stop> {
        let tmp = format!("{out_name}_tmp");
        let r = avw::fslroi(in_name, &tmp, &["0", "1"], self.fsl(), self.reproduce());
        let w = self.tool(r)?;
        self.int8_written(w, "fslroi", debug);
        self.created(&tmp);
        let mut w = vec![
            tmp.clone(),
            format!("{out_name}_tmp_brain"),
            "-m".into(),
            "-n".into(),
            "-f".into(),
            "0.3".into(),
        ];
        w.extend(opts.iter().cloned());
        self.bet2(&w)?;
        self.created(&format!("{out_name}_tmp_brain_mask"));
        self.created(&format!("{out_name}_tmp_brain"));
        let r = avw::fslmaths_dilf(
            &format!("{out_name}_tmp_brain_mask"),
            &format!("{out_name}_mask"),
            self.fsl(),
            self.reproduce(),
        );
        self.tool(r)?;
        let r = avw::fslmaths_mas(
            in_name,
            &format!("{out_name}_mask"),
            out_name,
            self.fsl(),
            self.reproduce(),
        );
        self.tool(r)?;
        Ok(())
    }

    /// `-Z`.
    fn small_fov(
        &mut self,
        in_name: &str,
        out_name: &str,
        opts: &[String],
        debug: bool,
    ) -> Result<(), Stop> {
        let val = |s: &mut Self, key: &str| -> Result<String, Stop> {
            let r = avw::fslval(in_name, key, s.fsl());
            Ok(s.tool(r)?.replace('-', ""))
        };
        let xdim = val(self, "dim1")?;
        let ydim = val(self, "dim2")?;
        let zdim = val(self, "dim3")?;
        let _xpix = val(self, "pixdim1")?;
        let _ypix = val(self, "pixdim2")?;
        let zpix = val(self, "pixdim3")?;
        let npad_text = dc::eval(&format!("100.0 {zpix} / {zdim} - 2 / 1 + p"), self.err);
        let npad: i64 = npad_text.trim().parse().unwrap_or(0);
        let mut words = vec![in_name.to_owned(), out_name.to_owned()];
        words.extend(opts.iter().cloned());
        if npad <= 0 {
            let _ = writeln!(
                self.out,
                "Image already has a greater Z FOV than 100.0 so not using padding"
            );
            self.bet2(&words)?;
            return Ok(());
        }
        let (xd, yd, zd) = (
            xdim.trim().to_owned(),
            ydim.trim().to_owned(),
            zdim.trim().to_owned(),
        );
        let bottom = format!("{out_name}_tmp_bottom");
        let top = format!("{out_name}_tmp_top");
        let r = avw::fslroi(
            in_name,
            &bottom,
            &["0", &xd, "0", &yd, "0", "1"],
            self.fsl(),
            self.reproduce(),
        );
        let w = self.tool(r)?;
        self.int8_written(w, "fslroi", debug);
        self.created(&bottom);
        let last = dc::eval(&format!("{zdim} 1 - p"), self.err);
        let r = avw::fslroi(
            in_name,
            &top,
            &["0", &xd, "0", &yd, last.trim(), "1"],
            self.fsl(),
            self.reproduce(),
        );
        let w = self.tool(r)?;
        self.int8_written(w, "fslroi", debug);
        self.created(&top);
        let mut inputs: Vec<String> = Vec::new();
        for _ in 0..npad {
            inputs.push(bottom.clone());
        }
        inputs.push(in_name.to_owned());
        for _ in 0..npad {
            inputs.push(top.clone());
        }
        let padded = format!("{out_name}_tmp");
        let r = avw::fslmerge_z(&padded, &inputs, self.fsl(), self.reproduce());
        let w = self.tool(r)?;
        self.int8_written(w, "fslmerge", debug);
        self.created(&padded);
        let mut w = vec![padded.clone(), out_name.to_owned()];
        w.extend(opts.iter().cloned());
        self.bet2(&w)?;
        // for f in `imglob ${OUT}*`: crop every image whose name starts with the output's
        let npad_s = npad.to_string();
        let globbed = avw::imglob(&avw::shell_glob_prefix(out_name));
        let ours: Vec<String> = globbed
            .iter()
            .filter(|f| self.is_our_output(f, out_name))
            .cloned()
            .collect();
        let imglob_found = which("imglob");
        let targets: Vec<String> = if self.reproduce() {
            if imglob_found {
                globbed.clone()
            } else {
                let _ = writeln!(
                    self.err,
                    "bet: line {IMGLOB_LINE}: imglob: command not found"
                );
                Vec::new()
            }
        } else {
            ours.clone()
        };
        let orig_would_crop: Vec<String> = if imglob_found {
            globbed.clone()
        } else {
            Vec::new()
        };
        // what stays to be seen: without -d the run's temporary files are removed whatever
        // happens to them (a run whose bet2 writes nothing, -n, changes nothing visible)
        let tmp_prefix = format!("{out_name}_tmp");
        let visible = |names: &[String]| -> Vec<String> {
            names
                .iter()
                .filter(|f| debug || !avw::remove_ext(f).starts_with(&tmp_prefix))
                .cloned()
                .collect()
        };
        if visible(&orig_would_crop) != visible(&ours) {
            let others: Vec<&str> = globbed
                .iter()
                .filter(|f| !ours.contains(f))
                .map(String::as_str)
                .collect();
            let detail = match (imglob_found, self.reproduce()) {
                (false, true) => "the outputs left padded: the script calls imglob without \
                                  ${FSLDIR}/bin, and FSL's programs are not on the PATH"
                    .to_owned(),
                (false, false) => "the outputs cropped back to the input's slices (the script \
                                   calls imglob without ${FSLDIR}/bin: with FSL's programs off \
                                   the PATH, as here, it leaves them padded)"
                    .to_owned(),
                (true, true) => format!(
                    "other images whose names start with the output's cropped too: {}",
                    others.join(", ")
                ),
                (true, false) => format!(
                    "other images whose names start with the output's left unchanged (the \
                     script crops them too): {}",
                    others.join(", ")
                ),
            };
            self.notices.add(&defects::FSL_45, detail);
        }
        for f in targets {
            let r = avw::fslroi(
                &f,
                &f,
                &["0", &xd, "0", &yd, &npad_s, &zd],
                self.fsl(),
                self.reproduce(),
            );
            let w = self.tool(r)?;
            self.int8_written(w, "fslroi", debug);
        }
        self.cleanup(out_name, debug);
        Ok(())
    }

    /// Whether an image name is one bet2 wrote for this output (the brain and its `_mask`,
    /// `_overlay`, `_skull`) or this run's temporary files.
    fn is_our_output(&self, f: &str, out_name: &str) -> bool {
        [
            out_name.to_owned(),
            format!("{out_name}_mask"),
            format!("{out_name}_overlay"),
            format!("{out_name}_skull"),
        ]
        .iter()
        .any(|o| o == f)
            || self.created.iter().any(|c| c == f)
    }
}

/// Whether bash would find `name` on the PATH.
fn which(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|d| {
        let p = d.join(name);
        p.is_file()
    })
}

/// bash's builtin `echo` (without `xpg_echo`) for the script's `echo $bet2opts` (FSL-63): the
/// leading words made only of `n`, `e` and `E` after a `-` are its own options (`-n`: no newline;
/// `-e` and `-E`: backslash escapes read or not, the last one deciding), the others printed
/// separated by blanks.
pub mod echo {
    /// What `echo words...` prints, and the words it took as its own options.
    pub fn run(words: &[String]) -> (String, Vec<String>) {
        let mut newline = true;
        let mut escapes = false;
        let mut taken = Vec::new();
        let mut rest = words;
        while let Some((w, tail)) = rest.split_first() {
            let flags = w.strip_prefix('-').unwrap_or("");
            if flags.is_empty() || !flags.chars().all(|c| matches!(c, 'n' | 'e' | 'E')) {
                break;
            }
            for c in flags.chars() {
                match c {
                    'n' => newline = false,
                    'e' => escapes = true,
                    _ => escapes = false,
                }
            }
            taken.push(w.clone());
            rest = tail;
        }
        let mut out = String::new();
        for (k, w) in rest.iter().enumerate() {
            if k > 0 {
                out.push(' ');
            }
            if escapes {
                let (text, stop) = unescape(w);
                out.push_str(&text);
                if stop {
                    return (out, taken);
                }
            } else {
                out.push_str(w);
            }
        }
        if newline {
            out.push('\n');
        }
        (out, taken)
    }

    /// `echo -e`'s escapes in one word (bash's `ansicstr` for echo): the text, and whether `\c`
    /// stopped all further output.
    fn unescape(w: &str) -> (String, bool) {
        let b: Vec<char> = w.chars().collect();
        let mut out = String::new();
        let mut i = 0;
        let digits = |i: &mut usize, max: usize, radix: u32| -> Option<u32> {
            let mut v: Option<u32> = None;
            let mut n = 0;
            while n < max {
                let Some(d) = b.get(*i).and_then(|c| c.to_digit(radix)) else {
                    break;
                };
                v = Some(v.unwrap_or(0) * radix + d);
                *i += 1;
                n += 1;
            }
            v
        };
        while i < b.len() {
            let c = b[i];
            i += 1;
            if c != '\\' || i >= b.len() {
                out.push(c);
                continue;
            }
            let e = b[i];
            i += 1;
            match e {
                'a' => out.push('\u{7}'),
                'b' => out.push('\u{8}'),
                'e' | 'E' => out.push('\u{1b}'),
                'f' => out.push('\u{c}'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                'v' => out.push('\u{b}'),
                '\\' => out.push('\\'),
                'c' => return (out, true),
                '0' => {
                    let v = digits(&mut i, 3, 8).unwrap_or(0) & 0xff;
                    out.push(char::from(v as u8));
                }
                'x' => match digits(&mut i, 2, 16) {
                    Some(v) => out.push(char::from(v as u8)),
                    None => out.push_str("\\x"),
                },
                'u' | 'U' => match digits(&mut i, if e == 'u' { 4 } else { 8 }, 16) {
                    Some(v) => out.push(char::from_u32(v).unwrap_or('\u{fffd}')),
                    None => {
                        out.push('\\');
                        out.push(e);
                    }
                },
                other => {
                    out.push('\\');
                    out.push(other);
                }
            }
        }
        (out, false)
    }

    #[cfg(test)]
    mod tests {
        use super::run;

        fn words(s: &str) -> Vec<String> {
            s.split(' ')
                .filter(|w| !w.is_empty())
                .map(str::to_owned)
                .collect()
        }

        #[test]
        fn leading_flag_words_are_echos_options() {
            assert_eq!(run(&words("-e -v")), ("-v\n".into(), words("-e")));
            assert_eq!(run(&words("-v -e")), ("-v -e\n".into(), vec![]));
            assert_eq!(run(&words("-n -e")), (String::new(), words("-n -e")));
            assert_eq!(run(&words("-ne -f 0.5")), ("-f 0.5".into(), words("-ne")));
            assert_eq!(run(&words("-E -m")), ("-m\n".into(), words("-E")));
            assert_eq!(run(&words("- -e")), ("- -e\n".into(), vec![]));
        }

        #[test]
        fn escapes_under_e() {
            assert_eq!(run(&words("-e a\\tb")).0, "a\tb\n");
            assert_eq!(run(&words("-e a\\cb -m")).0, "a");
            assert_eq!(run(&words("-e \\0101\\x41\\1\\q")).0, "AA\\1\\q\n");
            assert_eq!(run(&words("-E a\\tb")).0, "a\\tb\n");
        }
    }
}

/// GNU dc (1.4.1) for the expressions the script computes: numbers (`_` for a minus, read in
/// the input base), `k`, `+ - * / ^` and `p`, with bc's scale rules and truncation, and the
/// commands the numbers FSL's programs print can hold (`fslstats`' `nan`, `inf`, `1e+06`, a
/// minus sign: FSL-216): `n`, `f`, `a` (a string), `i`, and any other character's message.
pub mod dc {
    use std::io::Write;

    /// A decimal number: `mant * 10^-scale`.
    #[derive(Clone, Copy, Debug)]
    struct Num {
        mant: i128,
        scale: u32,
    }

    fn pow10(n: u32) -> i128 {
        10i128.pow(n.min(30))
    }

    fn rescale(a: Num, s: u32) -> Num {
        if s >= a.scale {
            Num {
                mant: a.mant * pow10(s - a.scale),
                scale: s,
            }
        } else {
            Num {
                mant: a.mant / pow10(a.scale - s), // truncation toward zero
                scale: s,
            }
        }
    }

    /// dc's printing (output base 10): zero as `0` at any scale, no leading `0` before the
    /// point (`.5`, `-.5`).
    fn show(a: Num) -> String {
        if a.mant == 0 {
            return "0".to_owned();
        }
        let neg = a.mant < 0;
        let m = a.mant.unsigned_abs().to_string();
        let s = a.scale as usize;
        let body = if s == 0 {
            m
        } else if m.len() > s {
            format!("{}.{}", &m[..m.len() - s], &m[m.len() - s..])
        } else {
            format!(".{}{}", "0".repeat(s - m.len()), m)
        };
        if neg && a.mant != 0 {
            format!("-{body}")
        } else {
            body
        }
    }

    /// A value on dc's stack.
    #[derive(Clone, Debug)]
    enum Val {
        N(Num),
        S(Vec<u8>),
    }

    fn print_val(v: &Val, out: &mut Vec<u8>) {
        match v {
            Val::N(n) => out.extend_from_slice(show(*n).as_bytes()),
            Val::S(s) => out.extend_from_slice(s),
        }
    }

    /// The digit `c` is in dc's input (0-9 and A-F, whatever the input base).
    fn digit(c: u8) -> Option<u32> {
        match c {
            b'0'..=b'9' => Some(u32::from(c - b'0')),
            b'A'..=b'F' => Some(u32::from(c - b'A') + 10),
            _ => None,
        }
    }

    /// Reads a number at `b[*i..]` (it starts with a digit, a point or `_`) in base `ibase`:
    /// the integer digits, then after a point the fraction's digits read as an integer and
    /// divided by `ibase` to their count, truncated to as many decimals.
    fn read_number(b: &[u8], i: &mut usize, ibase: u32) -> Num {
        let neg = b[*i] == b'_';
        if neg {
            *i += 1;
        }
        let base = i128::from(ibase);
        let mut int: i128 = 0;
        while let Some(d) = b.get(*i).copied().and_then(digit) {
            int = int.saturating_mul(base).saturating_add(i128::from(d));
            *i += 1;
        }
        let mut num = Num {
            mant: int,
            scale: 0,
        };
        if b.get(*i) == Some(&b'.') {
            *i += 1;
            let (mut frac, mut denom, mut n): (i128, i128, u32) = (0, 1, 0);
            while let Some(d) = b.get(*i).copied().and_then(digit) {
                frac = frac.saturating_mul(base).saturating_add(i128::from(d));
                denom = denom.saturating_mul(base);
                n += 1;
                *i += 1;
            }
            let f = frac.saturating_mul(pow10(n)) / denom;
            num = Num {
                mant: int.saturating_mul(pow10(n)).saturating_add(f),
                scale: n,
            };
        }
        if neg {
            num.mant = -num.mant;
        }
        num
    }

    /// Evaluates `expr` and returns what dc printed on its standard output (`p`, `n`, `f`;
    /// bytes that are not UTF-8 replaced), its own messages to `err`.
    pub fn eval(expr: &str, err: &mut dyn Write) -> String {
        let b = expr.as_bytes();
        let mut stack: Vec<Val> = Vec::new();
        let mut k: u32 = 0;
        let mut ibase: u32 = 10;
        let mut out: Vec<u8> = Vec::new();
        let mut i = 0;
        while i < b.len() {
            let c = b[i];
            if digit(c).is_some() || c == b'.' || c == b'_' {
                let n = read_number(b, &mut i, ibase);
                stack.push(Val::N(n));
                continue;
            }
            i += 1;
            let empty = |err: &mut dyn Write| {
                let _ = writeln!(err, "dc: stack empty");
            };
            match c {
                b' ' | b'\t' | b'\n' | b'\r' => {}
                b'p' => match stack.last() {
                    Some(v) => {
                        print_val(v, &mut out);
                        out.push(b'\n');
                    }
                    None => empty(err),
                },
                b'n' => match stack.pop() {
                    Some(v) => print_val(&v, &mut out),
                    None => empty(err),
                },
                b'f' => {
                    for v in stack.iter().rev() {
                        print_val(v, &mut out);
                        out.push(b'\n');
                    }
                }
                b'k' => match stack.pop() {
                    Some(Val::N(v)) if v.mant >= 0 => k = rescale(v, 0).mant as u32,
                    Some(_) => {
                        let _ = writeln!(err, "dc: scale must be a nonnegative number");
                    }
                    None => empty(err),
                },
                b'i' => match stack.pop() {
                    Some(Val::N(v)) if (2..=16).contains(&rescale(v, 0).mant) => {
                        ibase = rescale(v, 0).mant as u32;
                    }
                    Some(_) => {
                        let _ = writeln!(
                            err,
                            "dc: input base must be a number between 2 and 16 (inclusive)"
                        );
                    }
                    None => empty(err),
                },
                b'a' => match stack.pop() {
                    // the low-order byte of the integer part, as a one-character string
                    Some(Val::N(v)) => stack.push(Val::S(vec![(rescale(v, 0).mant & 0xff) as u8])),
                    Some(Val::S(s)) => stack.push(Val::S(s.into_iter().take(1).collect())),
                    None => empty(err),
                },
                b'+' | b'-' | b'*' | b'/' | b'^' => {
                    if stack.len() < 2 {
                        empty(err);
                        continue;
                    }
                    let (Val::N(a), Val::N(bv)) = (
                        stack[stack.len() - 2].clone(),
                        stack[stack.len() - 1].clone(),
                    ) else {
                        let _ = writeln!(err, "dc: non-numeric value");
                        continue;
                    };
                    let r = binop(c, a, bv, k);
                    match r {
                        Some(v) => {
                            stack.truncate(stack.len() - 2);
                            stack.push(Val::N(v));
                        }
                        None => {
                            let _ = writeln!(err, "dc: divide by zero");
                        }
                    }
                }
                other => {
                    // GNU dc prints its name on standard error and the rest on standard
                    // output, which the script's command substitution takes
                    let _ = write!(err, "dc: ");
                    out.extend_from_slice(
                        format!("'{}' (0{:o}) unimplemented\n", char::from(other), other)
                            .as_bytes(),
                    );
                }
            }
        }
        String::from_utf8_lossy(&out).into_owned()
    }

    /// `a op b` at scale `k` (bc's scale rules, truncated); `None` for a division by zero.
    fn binop(op: u8, a: Num, b: Num, k: u32) -> Option<Num> {
        Some(match op {
            b'+' | b'-' => {
                let s = a.scale.max(b.scale);
                let (a, b) = (rescale(a, s), rescale(b, s));
                let mant = if op == b'+' {
                    a.mant + b.mant
                } else {
                    a.mant - b.mant
                };
                Num { mant, scale: s }
            }
            b'*' => {
                let full = Num {
                    mant: a.mant * b.mant,
                    scale: a.scale + b.scale,
                };
                let s = (a.scale + b.scale).min(k.max(a.scale).max(b.scale));
                rescale(full, s)
            }
            b'/' => {
                if b.mant == 0 {
                    return None;
                }
                // a / b at scale k, truncated: (a * 10^(k + sb - sa)) / b
                let num = rescale(a, k + b.scale).mant;
                Num {
                    mant: num / b.mant,
                    scale: k,
                }
            }
            _ => {
                let e = rescale(b, 0).mant;
                if e >= 0 {
                    let e = e as u32;
                    let s = (a.scale * e).min(k.max(a.scale));
                    let mut full = Num { mant: 1, scale: 0 };
                    for _ in 0..e {
                        full = Num {
                            mant: full.mant * a.mant,
                            scale: full.scale + a.scale,
                        };
                    }
                    rescale(full, s)
                } else {
                    Num { mant: 0, scale: k }
                }
            }
        })
    }

    /// A number as C++'s iostream prints a double (`523.418`, `-3.25`, `1.23457e+06`,
    /// `1e-05`) written in dc's notation (`_` for a minus, the exponent applied: `_3.25`,
    /// `1234570`, `0.00001`); `None` for what is not a finite number (`nan`, `-nan`, `inf`).
    pub fn decimal(s: &str) -> Option<String> {
        let (neg, body) = match s.strip_prefix('-') {
            Some(r) => (true, r),
            None => (false, s.strip_prefix('+').unwrap_or(s)),
        };
        let (mant, exp) = match body.find(['e', 'E']) {
            Some(p) => (&body[..p], body[p + 1..].parse::<i32>().ok()?),
            None => (body, 0),
        };
        let (ip, fp) = mant.split_once('.').unwrap_or((mant, ""));
        if ip.is_empty() && fp.is_empty()
            || !ip.bytes().all(|c| c.is_ascii_digit())
            || !fp.bytes().all(|c| c.is_ascii_digit())
        {
            return None;
        }
        let digits = format!("{ip}{fp}");
        // the point after `point` digits
        let point = ip.len() as i64 + i64::from(exp);
        let (int, frac) = if point <= 0 {
            (
                "0".to_owned(),
                format!("{}{digits}", "0".repeat((-point) as usize)),
            )
        } else if point as usize >= digits.len() {
            (
                format!("{digits}{}", "0".repeat(point as usize - digits.len())),
                String::new(),
            )
        } else {
            (
                digits[..point as usize].to_owned(),
                digits[point as usize..].to_owned(),
            )
        };
        let int = int.trim_start_matches('0');
        let int = if int.is_empty() { "0" } else { int };
        let mut out = String::new();
        if neg {
            out.push('_');
        }
        out.push_str(int);
        if !frac.is_empty() {
            out.push('.');
            out.push_str(&frac);
        }
        Some(out)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn the_scripts_expressions() {
            let mut e = Vec::new();
            assert_eq!(eval("100.0 4.000000  / 34  - 2 / 1 + p", &mut e), "-3\n");
            assert_eq!(eval("100.0 4.000000  / 18  - 2 / 1 + p", &mut e), "4\n");
            assert_eq!(eval("1 1 + p", &mut e), "2\n");
            assert_eq!(eval("_10 1 + p", &mut e), "-9\n");
            assert_eq!(
                eval(
                    "0 k 31.5 31.25 - 2 ^ 28.25 28.0 - 2 ^ + 16.0 16.0 - 2 ^ + 1 / p",
                    &mut e
                ),
                "0\n"
            );
            assert_eq!(
                eval(
                    "0 k 31.517222 30.012345 - 2 ^ 1.0 0 - 2 ^ + 0 0 - 2 ^ + 1 / p",
                    &mut e
                ),
                "3\n"
            );
            assert!(e.is_empty());
        }

        /// What GNU dc 1.4.1 prints (standard output, standard error) for the numbers fslstats
        /// can print in `bet -S`'s threshold (FSL-216) and the commands they hold (run
        /// 2026-11-04).
        #[test]
        fn gnu_dc_on_what_fslstats_prints() {
            let cases: &[(&str, &str, &str)] = &[
                (
                    "10 k 3.5 1.23457e+06 * 5.5 + p",
                    "'e' (0145) unimplemented\n33.90742\n",
                    "dc: ",
                ),
                (
                    "10 k 3.5 nan * 5.5 + p",
                    "3.55.5\n",
                    "dc: stack empty\ndc: stack empty\ndc: stack empty\ndc: stack empty\n",
                ),
                (
                    "10 k 3.5 -nan * 5.5 + p",
                    "3.55.5\n",
                    "dc: stack empty\ndc: stack empty\ndc: stack empty\ndc: stack empty\ndc: stack empty\n",
                ),
                (
                    "10 k 3.5 inf * 5.5 + p",
                    "6.6\n",
                    "dc: stack empty\ndc: stack empty\ndc: stack empty\n",
                ),
                (
                    "10 k 3.5 12.5  * -3.25  + p",
                    "47.00\n",
                    "dc: stack empty\n",
                ),
                ("10 k 3.5 81.2345  * 523.418  + p", "807.73875\n", ""),
                ("10 k 3.5 0  * 0  + p", "0\n", ""),
                ("3.5 a p", "\u{3}\n", ""),
                ("300 a p f", ",\n,\n", ""),
                ("_3 a f", "\u{fffd}\n", ""),
                ("3.5 i 5.5 p", "6.6\n", ""),
                (
                    "1 i 5 p",
                    "5\n",
                    "dc: input base must be a number between 2 and 16 (inclusive)\n",
                ),
                (
                    "0 k 3.5 a 2 * p f",
                    "2\n2\n\u{3}\n",
                    "dc: non-numeric value\n",
                ),
                (
                    "10 k 3.5 nan * nan + p",
                    "3.5",
                    &"dc: stack empty\n".repeat(8),
                ),
                ("10 k 3.5 inf * inf + p", "", &"dc: stack empty\n".repeat(6)),
                (
                    "_2 k 1 p",
                    "1\n",
                    "dc: scale must be a nonnegative number\n",
                ),
                ("3 i 5.55 p", "7.22\n", ""),
                ("16 i A.8 p", "10.5\n", ""),
                (
                    "10 k 3.5 1e-05 * 2 + p",
                    "'e' (0145) unimplemented\n14.5\n",
                    "dc: ",
                ),
                (
                    "10 k 3.5 0.5 * -nan + p",
                    "1.75",
                    &"dc: stack empty\n".repeat(5),
                ),
                ("1.2.3 f", ".3\n1.2\n", ""),
            ];
            for (expr, out, err) in cases {
                let mut e = Vec::new();
                assert_eq!(eval(expr, &mut e), *out, "{expr}");
                assert_eq!(String::from_utf8(e).unwrap(), *err, "{expr}");
            }
        }

        #[test]
        fn numbers_as_printed_in_dcs_notation() {
            for (s, d) in [
                ("523.418", Some("523.418")),
                ("-3.25", Some("_3.25")),
                ("1.23457e+06", Some("1234570")),
                ("1e-05", Some("0.00001")),
                ("2.5e-07", Some("0.00000025")),
                ("0", Some("0")),
                ("-0", Some("_0")),
                ("12345.6", Some("12345.6")),
                ("nan", None),
                ("-nan", None),
                ("inf", None),
                ("-inf", None),
                ("", None),
            ] {
                assert_eq!(decimal(s).as_deref(), d, "{s}");
            }
            let mut e = Vec::new();
            assert_eq!(
                eval("10 k 3.5 0.00001 * 1234570 + p", &mut e),
                "1234570.000035\n"
            );
            assert!(e.is_empty());
        }

        /// A centre printed as `-0.000000` (FSL 6.0.7.23's `fslstats -C`): dc reads the `-` as a
        /// subtraction (GNU dc 1.4.1's outputs and messages, run 2026-10-29).
        #[test]
        fn minus_zero_is_a_subtraction() {
            let mut e = Vec::new();
            assert_eq!(eval("0 k 1.0 -0.000000 - 2 ^ p", &mut e), "1.000000\n");
            assert_eq!(
                eval("100.0 4.000000  / -0.000000  - 2 / 1 + p", &mut e),
                "13\n"
            );
            assert_eq!(String::from_utf8(e).unwrap(), "dc: stack empty\n".repeat(2));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_bet(settings: &Settings, args: &[&str]) -> (u8, String, String) {
        let run = Run {
            fsl: settings,
            mode: UpstreamDefects::Fix,
            threads: 1,
            fill: true,
        };
        let argv: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = main(&argv, &run, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    /// `-S`, `-B`, `-A` and `-A2` refuse before running anything when a standard image they
    /// read is nowhere (here: not in `LARMORX_FSL_DATA`), naming it.
    #[test]
    fn missing_standard_images_are_refused_first() {
        let d = tempfile::tempdir().unwrap();
        let dir = d.path().to_string_lossy().into_owned();
        let input = format!("{dir}/in.nii.gz");
        std::fs::write(&input, b"not read").unwrap();
        let data = d.path().join("data");
        std::fs::create_dir_all(data.join("standard")).unwrap();
        let s = Settings::new()
            .with(fsldata::ENV, &data.to_string_lossy())
            .with("FSLOUTPUTTYPE", "NIFTI_GZ");
        let out = format!("{dir}/out");
        for v in [&["-S"][..], &["-B"], &["-A"], &["-A2", "t2"]] {
            let mut args = vec!["bet", input.as_str(), out.as_str()];
            args.extend_from_slice(v);
            let (code, stdout, stderr) = run_bet(&s, &args);
            assert_eq!(code, 1, "{v:?}: {stderr}");
            assert!(stdout.is_empty(), "{v:?}: {stdout}");
            assert!(
                stderr.starts_with(
                    "larmorx-nc: error: bet: FSL's data image standard/MNI152_T1_2mm is not in \
                     LARMORX_FSL_DATA="
                ),
                "{v:?}: {stderr}"
            );
        }
        // with MNI152 and its dilated mask there, -S names the eye mask
        for f in ["MNI152_T1_2mm", "MNI152_T1_2mm_brain_mask_dil"] {
            std::fs::write(data.join("standard").join(format!("{f}.nii.gz")), b"x").unwrap();
        }
        let (code, _, stderr) = run_bet(&s, &["bet", &input, &out, "-S"]);
        assert_eq!(code, 1);
        assert!(
            stderr.contains("FSL's data image standard/MNI152_T1_2mm_eye_mask is not in"),
            "{stderr}"
        );
        let mut left: Vec<String> = std::fs::read_dir(d.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(left, ["data", "in.nii.gz"], "nothing written");
    }

    /// `-S`'s threshold (FSL-216): plain numbers as FSL's dc computes them; an exponent read
    /// as a number by default, as dc's commands in `reproduce`; a NaN refused by default.
    #[test]
    fn eye_threshold_fsl_216() {
        let s = Settings::new();
        let thr = |mode: UpstreamDefects, mean: &str, sd: &str| {
            let run = Run {
                fsl: &s,
                mode,
                threads: 1,
                fill: false,
            };
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let mut sc = Script {
                run: &run,
                out: &mut out,
                err: &mut err,
                notices: Notices::new(mode),
                args: Vec::new(),
                created: Vec::new(),
                before: Vec::new(),
            };
            let r = sc.eye_threshold(mean, sd).ok();
            let named = sc.notices.contains(&defects::FSL_216);
            (r, named, String::from_utf8(err).unwrap())
        };
        let (fix, re) = (UpstreamDefects::Fix, UpstreamDefects::Reproduce);
        assert_eq!(
            thr(fix, "523.418 ", "81.2345 "),
            (Some("807.73875".into()), false, String::new())
        );
        assert_eq!(
            thr(re, "523.418 ", "81.2345 "),
            (Some("807.73875".into()), false, String::new())
        );
        assert_eq!(
            thr(fix, "2.1e+06 ", "81.2345 "),
            (Some("2100284.32075".into()), true, String::new())
        );
        assert_eq!(
            thr(re, "2.1e+06 ", "81.2345 "),
            (
                Some("'e' (0145) unimplemented\n292.42075".into()),
                true,
                "dc: ".into()
            )
        );
        assert_eq!(
            thr(fix, "-12.5 ", "2 "),
            (Some("-5.5".into()), true, String::new())
        );
        let (r, named, err) = thr(fix, "nan ", "nan ");
        assert!(
            r.is_none() && named && err.contains("is not a number"),
            "{err}"
        );
        let (r, named, _) = thr(re, "nan ", "nan ");
        assert_eq!((r.as_deref(), named), (Some("3.5"), true));
    }
}
