// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-flirt 2111.4 (the script standard_space_roi).
// Copyright (C) 2005-2007 University of Oxford (Stephen Smith). Rust translation Copyright 2026
// Karel Lopez Vilaret; modified from the original. Distributed under the FSL Licence (see
// LICENSE).
//! `standard_space_roi <input> <output> [options] [flirt options]`: the input registered to a
//! standard-space reference with FLIRT, the reference's field of view (less its outer voxel) or
//! a standard-space mask taken back into the input's space through the inverse matrix, and the
//! input masked with it and/or cut down to its bounding box. The script's programs are the
//! replica's own, run in this process: `flirt`, `convert_xfm`, `fslval`, `fslmaths`, `fslstats
//! -w`, `fslroi`, fslpy's `imcp`, GNU `dc` and `rm`.
//!
//! As the script (bash, no `-e`): each step's messages in order, the shell's own (`[: too many
//! arguments`, a crashed program's report), and the exit status of its last command (the
//! cleanup's `rm`, or 0 with `-d`). FSL's defects, corrected by default and reproduced with
//! `reproduce`: FSL-211 (unquoted names), FSL-212 (the cleanup's glob), FSL-213 (`maskthr` from
//! the environment), FSL-214 (failures do not stop it).
//!
//! The standard images (`${FSLDIR}/data/standard/MNI152_T1_2mm` and, for `-b`,
//! `MNI152_T1_2mm_brain_mask_dil`) are found by [`larmorx_nc_core::fsldata`]; the script
//! refuses before it runs anything when one it needs is nowhere.

use std::io::Write;

use larmorx_core::defects::{self, Notices, UpstreamDefects};
use larmorx_nc_core::avwutils as avw;
use larmorx_nc_core::exception;
use larmorx_nc_core::fsldata;
use larmorx_nc_core::fslpy;
use larmorx_nc_core::settings::Settings;

use crate::run::PREFIX;

/// The script's name, as bash names it in its messages (`$0`: FSL's full path; the suites read
/// it as the name).
const NAME: &str = "standard_space_roi";

/// The standard images the script names (relative to `${FSLDIR}/data`).
const STANDARD: &str = "standard/MNI152_T1_2mm";
const BRAIN_MASK_DIL: &str = "standard/MNI152_T1_2mm_brain_mask_dil";

/// fslpy's `remove_ext` without arguments: its usage on standard output, captured by the
/// script's backticks.
const REMOVE_EXT_USAGE: &str = "Usage: remove_ext <list of image paths to remove extension from>";

fn usage_text(fsldir: &str) -> String {
    format!(
        "
Usage: standard_space_roi <input> <output> [options] [flirt options]

This masks the input and/or reduces its FOV, on the basis of a
standard space image or mask, that is transformed into the space
of the input image.

Masking options (applied before FOV-reduction):
  -maskFOV           : mask output using transformed standard space FOV (default)
  -maskMASK <mask>   : mask output using transformed standard space mask
  -maskNONE          : do not mask output

FOV-reduction options:
  -roiFOV            : cut down input FOV using bounding box of the transformed standard space FOV (default)
  -roiMASK <mask>    : cut down input FOV using nonbackground bounding box of the transformed standard space mask
  -roiNONE           : do not cut down input FOV

With either MASK option, <mask> must be in the same space as the standard space reference image.

Other options:
  -ssref <ref_image> : standard space reference image to use (default {fsldir}/data/standard/MNI152_T1)
  -altinput <image>  : alternative input image to apply the ROI to (instead of the one used to register to the reference)
  -d                 : debug (don't delete intermediate files)
  -b                 : betpremask, equivalent to: -maskMASK {fsldir}/data/standard/MNI152_T1_2mm_brain_mask_dil -roiNONE

All above options must come before any flirt options.

"
    )
}

/// How the script stops.
enum Stop {
    /// `exit <code>`, or the end with this status.
    Exit(u8),
}

/// A name the script uses: one the arguments give, or one of FSL's data images (found later).
#[derive(Clone)]
enum Name {
    Given(String),
    Data(&'static str),
}

/// What the script's commands are: a program of the replica.
type Program =
    fn(&[String], &Settings, UpstreamDefects, usize, &mut dyn Write, &mut dyn Write) -> u8;

/// The line bash names in its report of a crashed command: the command's own at the top
/// level, the end (`fi`) of the `if` block it runs in.
fn reported_line(line: u32) -> u32 {
    match line {
        219..=229 => 229,
        232..=242 => 242,
        _ => line,
    }
}

/// The words of a shell word that holds unquoted expansions (IFS: blank, tab, newline).
fn split_words(v: &str) -> Vec<String> {
    v.split([' ', '\t', '\n'])
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

struct Script<'a, 'w> {
    fsl: &'a Settings,
    mode: UpstreamDefects,
    threads: usize,
    out: &'w mut dyn Write,
    err: &'w mut dyn Write,
    notices: Notices,
    /// The temporary files this run's programs wrote (by name, without an image's extension).
    created: Vec<String>,
}

/// `standard_space_roi <input> <output> [options] [flirt options]`.
pub fn standard_space_roi(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut s = Script {
        fsl: settings,
        mode,
        threads,
        out,
        err,
        notices: Notices::new(mode),
        created: Vec::new(),
    };
    let code = match s.body(args) {
        Ok(c) | Err(Stop::Exit(c)) => c,
    };
    let _ = s.out.flush();
    for n in s.notices.as_slice() {
        let _ = writeln!(s.err, "{PREFIX}: {}", n.line());
    }
    code
}

impl Script<'_, '_> {
    fn reproduce(&self) -> bool {
        self.mode.reproduce()
    }

    fn usage(&mut self) -> Stop {
        let fsldir = self.fsl.get("FSLDIR").unwrap_or("").to_owned();
        let _ = self.out.write_all(usage_text(&fsldir).as_bytes());
        Stop::Exit(1)
    }

    /// The script's `[ <prefix>$v <op> <rhs> ]`: by default the value as one word; in
    /// `reproduce` its words as bash's `test` sees them (FSL-211), with `[`'s message and its
    /// line where they make no test (the test is then false).
    fn sh_test(&mut self, prefix: &str, v: &str, op: &str, rhs: &str, line: u32) -> bool {
        if !self.reproduce() {
            let word = format!("{prefix}{v}");
            return if op == "=" { word == rhs } else { word != rhs };
        }
        let mut w = split_words(&format!("{prefix}{v}"));
        w.extend([op.to_owned(), rhs.to_owned()]);
        match crate::shtest::test(&w) {
            Ok(v) => v,
            Err(m) => {
                let _ = writeln!(self.err, "{NAME}: line {line}: [: {m}");
                false
            }
        }
    }

    /// The words of an unquoted `$name` (FSL-211): split at blanks in `reproduce`, the name as
    /// one word by default.
    fn words(&mut self, v: &str, what: &str) -> Vec<String> {
        let split = split_words(v);
        if split.len() == 1 && split[0] == v {
            return split;
        }
        if self.reproduce() {
            if !v.is_empty() {
                self.notices.add(
                    &defects::FSL_211,
                    format!(
                        "{what} '{v}' split into {} word(s), as the script does",
                        split.len()
                    ),
                );
            }
            return split;
        }
        if !v.is_empty() {
            self.notices.add(
                &defects::FSL_211,
                format!("{what} '{v}' used as one name (the script splits it into words)"),
            );
        }
        vec![v.to_owned()]
    }

    /// The name of `n` as the commands get it: a data image found ([`fsldata::find`]; checked
    /// before the work starts) as one word, a given name as its words.
    fn name_words(&mut self, n: &Name, what: &str) -> Vec<String> {
        match n {
            Name::Given(v) => self.words(&v.clone(), what),
            Name::Data(rel) => vec![
                fsldata::find(rel, self.fsl)
                    .map(|f| f.name)
                    .unwrap_or_default(),
            ],
        }
    }

    /// The words of `$v` without a notice: split in `reproduce`, the value by default.
    fn words_quiet(&self, v: &str) -> Vec<String> {
        if self.reproduce() {
            split_words(v)
        } else {
            vec![v.to_owned()]
        }
    }

    /// `[ `imtest $v` = 0 ]`: imtest prints 0 unless it gets exactly one word naming an image.
    /// FSL-211 is named where the split changes the answer.
    fn imtest(&mut self, v: &str, what: &str) -> bool {
        let whole = avw::imtest(v);
        let split = split_words(v);
        let pieces = split.len() == 1 && avw::imtest(&split[0]);
        if whole != pieces {
            let detail = if self.reproduce() {
                format!(
                    "{what} '{v}' split into {} word(s): not valid, as the script finds",
                    split.len()
                )
            } else {
                format!(
                    "{what} '{v}' read as one name (the script splits it into words and finds it not valid)"
                )
            };
            self.notices.add(&defects::FSL_211, detail);
        }
        if self.reproduce() { pieces } else { whole }
    }

    /// Runs one of the script's programs in this process: its own messages, its exit status;
    /// an exception nothing in it catches aborts it (`std::terminate`'s text and bash's report,
    /// status 134). With the defects corrected, a failure stops the script (FSL-214).
    fn program(
        &mut self,
        line: u32,
        source: &str,
        name: &str,
        f: Program,
        args: Vec<String>,
    ) -> Result<u8, Stop> {
        let (fsl, mode, threads) = (self.fsl, self.mode, self.threads);
        let (r, aborted) = {
            let (out, err) = (&mut *self.out, &mut *self.err);
            // a program of FLIRT's that ends where FSL's process aborts says so on its own line
            let mut watch = exception::AbortWatch::new(err);
            let r = exception::catch(|| f(&args, fsl, mode, threads, out, &mut watch));
            let aborted = watch.aborted();
            (r, aborted)
        };
        let report = |s: &mut Self| {
            let _ = writeln!(
                s.err,
                "{NAME}: line {}: {:>5} Aborted                 {source}",
                reported_line(line),
                std::process::id()
            );
        };
        let code = match r {
            Ok(_) if aborted => {
                // its text is printed; bash's report of the aborted command follows
                report(self);
                134
            }
            Ok(code) => code,
            Err(e) => {
                let _ = write!(self.err, "{}", e.stderr_text());
                report(self);
                134
            }
        };
        let _ = self.out.flush();
        self.check(name, code)?;
        Ok(code)
    }

    /// FSL-214: by default a failed step stops the script (its temporary files removed unless
    /// `-d`); FSL's script goes on.
    fn check(&mut self, name: &str, code: u8) -> Result<(), Stop> {
        if code == 0 {
            return Ok(());
        }
        if self.reproduce() {
            self.notices.add(
                &defects::FSL_214,
                format!("{name} failed (exit status {code}); the script goes on, as FSL's"),
            );
            return Ok(());
        }
        self.notices.add(
            &defects::FSL_214,
            format!(
                "{name} failed (exit status {code}): stopped with exit status 1 (FSL's script \
                 goes on with the next steps and exits with its cleanup's status)"
            ),
        );
        Err(Stop::Exit(1))
    }

    fn body(&mut self, a: &[String]) -> Result<u8, Stop> {
        // the shell would expand these as file-name patterns: not reproduced
        if self.reproduce()
            && let Some(w) = a.iter().find(|w| w.contains(['*', '?', '[']))
        {
            let _ = writeln!(
                self.err,
                "{PREFIX}: error: {NAME}: the script expands its arguments unquoted, so the \
                 shell would read {w:?} as a file-name pattern; that is not reproduced (FSL-211)"
            );
            return Err(Stop::Exit(1));
        }
        let arg = |k: usize| a.get(k).map_or("", String::as_str);
        // [ _$2 = _ ] && Usage
        if self.sh_test("_", arg(1), "=", "_", 112) {
            return Err(self.usage());
        }
        // in=`${FSLDIR}/bin/remove_ext $1`
        let in_words = self.words_quiet(arg(0));
        // (an empty input: remove_ext prints its usage, which the script takes as the input's
        // name in `reproduce`; both modes refuse it, only the message differs: no notice)
        let input = if in_words.is_empty() {
            REMOVE_EXT_USAGE.to_owned()
        } else {
            in_words
                .iter()
                .map(|w| avw::remove_ext(w))
                .collect::<Vec<_>>()
                .join(" ")
        };
        if !self.imtest(&input, "the input") {
            let _ = writeln!(self.out, "Input image {input} not valid");
            return Err(Stop::Exit(1));
        }
        // out=`${FSLDIR}/bin/remove_ext $1` (the second argument)
        let out_words = self.words_quiet(arg(1));
        let output = out_words
            .iter()
            .map(|w| avw::remove_ext(w))
            .collect::<Vec<_>>()
            .join(" ");

        let mut debug = false;
        let mut standard = Name::Data(STANDARD);
        let mut inapply = Name::Given(input.clone());
        let mut roi = 1;
        let mut mask = 1;
        // maskthr: unset by the script except for -b; bash takes it from the environment
        let env_maskthr = self.fsl.get("maskthr").map(str::to_owned);
        let mut maskthr: Option<String> = None;
        let mut maskmask: Option<Name> = None;
        let mut roimask: Option<String> = None;
        let mut i = 2;
        while i < a.len() {
            let o = arg(i);
            if !self.sh_test("_", o, "!=", "_", 133) {
                break;
            }
            let value = |s: &mut Self, line: u32| -> Result<String, Stop> {
                let v = arg(i + 1);
                if s.sh_test("_", v, "=", "_", line) {
                    return Err(s.usage());
                }
                Ok(v.to_owned())
            };
            // the if/elif chain: `[ $1 = -d ]`, `[ $1 = -b ]`, ...: the first that holds
            const CHAIN: [(&str, u32); 10] = [
                ("-d", 135),
                ("-b", 138),
                ("-ssref", 144),
                ("-altinput", 152),
                ("-roiFOV", 161),
                ("-roiMASK", 164),
                ("-roiNONE", 173),
                ("-maskFOV", 177),
                ("-maskMASK", 180),
                ("-maskNONE", 189),
            ];
            let mut chosen = "";
            for (opt, line) in CHAIN {
                if self.sh_test("", o, "=", opt, line) {
                    chosen = opt;
                    break;
                }
            }
            match chosen {
                "-d" => {
                    debug = true;
                    i += 1;
                }
                "-b" => {
                    maskthr = Some("-thr 0.5".to_owned());
                    mask = 2;
                    maskmask = Some(Name::Data(BRAIN_MASK_DIL));
                    roi = 3;
                    i += 1;
                }
                "-ssref" => {
                    let v = value(self, 145)?;
                    i += 2;
                    if !self.imtest(&v, "the reference") {
                        let _ = writeln!(self.out, "standard space reference image not valid");
                        return Err(Stop::Exit(1));
                    }
                    standard = Name::Given(v);
                }
                "-altinput" => {
                    let v = value(self, 153)?;
                    i += 2;
                    if !self.imtest(&v, "the alternative input") {
                        let _ = writeln!(self.out, "Input image {v} not valid");
                        return Err(Stop::Exit(1));
                    }
                    inapply = Name::Given(v);
                }
                "-roiFOV" => {
                    roi = 1;
                    i += 1;
                }
                "-roiMASK" => {
                    roi = 2;
                    let v = value(self, 166)?;
                    i += 2;
                    if !self.imtest(&v, "the ROI mask") {
                        let _ = writeln!(self.out, "Image {v} not valid");
                        return Err(Stop::Exit(1));
                    }
                    roimask = Some(v);
                }
                "-roiNONE" => {
                    roi = 3;
                    i += 1;
                }
                "-maskFOV" => {
                    mask = 1;
                    i += 1;
                }
                "-maskMASK" => {
                    mask = 2;
                    let v = value(self, 182)?;
                    i += 2;
                    if !self.imtest(&v, "the mask") {
                        let _ = writeln!(self.out, "Image {v} not valid");
                        return Err(Stop::Exit(1));
                    }
                    maskmask = Some(Name::Given(v));
                }
                "-maskNONE" => {
                    mask = 3;
                    i += 1;
                }
                _ => break,
            }
        }
        // FSL-213: the environment's maskthr where the script does not set it
        if maskthr.is_none()
            && let Some(m) = env_maskthr.filter(|m| !split_words(m).is_empty())
            && mask < 3
        {
            if self.reproduce() {
                self.notices.add(
                    &defects::FSL_213,
                    format!("the environment's maskthr '{m}' applied to the mask, as FSL's"),
                );
                maskthr = Some(m);
            } else {
                self.notices.add(
                    &defects::FSL_213,
                    format!("the environment's maskthr '{m}' ignored (FSL applies it)"),
                );
            }
        }
        // the data images the work needs, before anything runs
        let mut needed = vec![&standard];
        if mask == 2
            && let Some(m) = &maskmask
        {
            needed.push(m);
        }
        for n in needed {
            if let Name::Data(rel) = n
                && let Err(e) = fsldata::find(rel, self.fsl)
            {
                let _ = writeln!(self.err, "{PREFIX}: error: {NAME}: {e}");
                return Err(Stop::Exit(1));
            }
        }
        // "$@" split again by the unquoted $@ (FSL-211)
        let mut flirt_opts: Vec<String> = Vec::new();
        for o in &a[i.min(a.len())..] {
            if self.reproduce() {
                let w = split_words(o);
                if !(w.len() == 1 && w[0] == *o) && !o.is_empty() {
                    self.notices.add(
                        &defects::FSL_211,
                        format!(
                            "FLIRT's option '{o}' split into {} word(s), as the script does",
                            w.len()
                        ),
                    );
                }
                flirt_opts.extend(w);
            } else {
                flirt_opts.push(o.clone());
            }
        }
        let code = self.work(
            &input,
            &output,
            &standard,
            &inapply,
            roi,
            mask,
            maskthr,
            maskmask,
            roimask,
            &flirt_opts,
        );
        let code = match code {
            Ok(()) => None,
            Err(Stop::Exit(c)) => Some(c),
        };
        // cleanup (also after a stop, unless -d)
        let status = if debug { 0 } else { self.cleanup(&output) };
        Ok(code.unwrap_or(status))
    }

    /// `${out}_<suffix>` as the commands get it (its words).
    fn tmp(&mut self, output: &str, suffix: &str) -> Vec<String> {
        let w = self.words(&format!("{output}_{suffix}"), "the output");
        self.created.push(format!("{output}_{suffix}"));
        w
    }

    #[allow(clippy::too_many_arguments)]
    fn work(
        &mut self,
        input: &str,
        output: &str,
        standard: &Name,
        inapply: &Name,
        roi: u8,
        mask: u8,
        maskthr: Option<String>,
        maskmask: Option<Name>,
        roimask: Option<String>,
        flirt_opts: &[String],
    ) -> Result<(), Stop> {
        let one = |s: &str| vec![s.to_owned()];
        let cat =
            |parts: Vec<Vec<String>>| -> Vec<String> { parts.into_iter().flatten().collect() };
        let ins = self.words(input, "the input");
        let std_words = self.name_words(standard, "the reference");
        // flirt -in $in -ref $standard -o ${out}_tmp_to_std -omat ${out}_tmp_to_std.mat $@
        let to_std = self.tmp(output, "tmp_to_std");
        let to_std_mat = self.tmp(output, "tmp_to_std.mat");
        let args = cat(vec![
            one("-in"),
            ins.clone(),
            one("-ref"),
            std_words.clone(),
            one("-o"),
            to_std,
            one("-omat"),
            to_std_mat.clone(),
            flirt_opts.to_vec(),
        ]);
        self.program(
            211,
            "${FSLDIR}/bin/flirt -in $in -ref $standard -o ${out}_tmp_to_std -omat ${out}_tmp_to_std.mat $@",
            "flirt",
            crate::flirt,
            args,
        )?;
        // convert_xfm -inverse -omat ${out}_tmp_to_std_inv.mat ${out}_tmp_to_std.mat
        let inv = self.tmp(output, "tmp_to_std_inv.mat");
        let args = cat(vec![one("-inverse"), one("-omat"), inv.clone(), to_std_mat]);
        self.program(
            212,
            "${FSLDIR}/bin/convert_xfm -inverse -omat ${out}_tmp_to_std_inv.mat ${out}_tmp_to_std.mat",
            "convert_xfm",
            crate::convert_xfm,
            args,
        )?;
        // X=`fslval $standard dim1`; X=`echo "$X 2 - p" | dc` (and Y, Z)
        let mut xyz = Vec::new();
        for key in ["dim1", "dim2", "dim3"] {
            // fslval's own script quotes "$1": a split name gives it other arguments
            let v = if std_words.len() == 1 {
                match avw::fslval(&std_words[0], key, self.fsl) {
                    Ok(v) => v,
                    Err(f) => {
                        let _ = self.err.write_all(f.err.as_bytes());
                        self.check("fslval", f.code.max(1))?;
                        String::new()
                    }
                }
            } else {
                String::new()
            };
            let v = v.trim_end_matches('\n');
            let n = crate::scripts::dc(&format!("{v} 2 - p"), self.err);
            xyz.push(n.map_or_else(String::new, |n| n.to_string()));
        }
        let fov = |s: &mut Self, std_words: &[String], dest: Vec<String>, line: u32| {
            // fslmaths $standard -mul 0 -add 1 -roi 1 $X 1 $Y 1 $Z 0 1 <dest> -odt float
            let mut args = std_words.to_vec();
            args.extend(
                [
                    "-mul", "0", "-add", "1", "-roi", "1", &xyz[0], "1", &xyz[1], "1", &xyz[2],
                    "0", "1",
                ]
                .iter()
                .filter(|w| !w.is_empty())
                .map(|w| (*w).to_owned()),
            );
            args.extend(dest);
            args.extend(["-odt".to_owned(), "float".to_owned()]);
            let source = if line == 221 {
                "${FSLDIR}/bin/fslmaths $standard -mul 0 -add 1 -roi 1 $X 1 $Y 1 $Z 0 1 ${out}_tmp_standard_mask -odt float"
            } else {
                "${FSLDIR}/bin/fslmaths $standard -mul 0 -add 1 -roi 1 $X 1 $Y 1 $Z 0 1 ${out}_tmp_standard_roi -odt float"
            };
            s.program(
                line,
                source,
                "fslmaths",
                larmorx_nc_avwutils::fslmaths,
                args,
            )
        };
        let mut inapply_words = self.name_words(inapply, "the alternative input");
        // first do masking, if requested
        if mask < 3 {
            let dest = self.tmp(output, "tmp_standard_mask");
            if mask == 1 {
                fov(self, &std_words, dest.clone(), 221)?;
            } else {
                let m = maskmask.clone().unwrap_or(Name::Given(String::new()));
                let mw = self.name_words(&m, "the mask");
                let args = cat(vec![
                    mw,
                    one("-bin"),
                    dest.clone(),
                    one("-odt"),
                    one("float"),
                ]);
                self.program(
                    223,
                    "${FSLDIR}/bin/fslmaths $maskmask -bin ${out}_tmp_standard_mask -odt float",
                    "fslmaths",
                    larmorx_nc_avwutils::fslmaths,
                    args,
                )?;
            }
            let to_in = self.tmp(output, "tmp_standard_mask_to_in");
            let args = cat(vec![
                one("-ref"),
                ins.clone(),
                one("-in"),
                dest,
                one("-o"),
                to_in.clone(),
                one("-applyxfm"),
                one("-init"),
                inv.clone(),
            ]);
            self.program(
                225,
                "${FSLDIR}/bin/flirt -ref $in -in ${out}_tmp_standard_mask -o ${out}_tmp_standard_mask_to_in -applyxfm -init ${out}_tmp_to_std_inv.mat",
                "flirt",
                crate::flirt,
                args,
            )?;
            let thr = maskthr.as_deref().map(split_words).unwrap_or_default();
            let args = cat(vec![to_in.clone(), thr, to_in.clone()]);
            self.program(
                226,
                "${FSLDIR}/bin/fslmaths ${out}_tmp_standard_mask_to_in $maskthr ${out}_tmp_standard_mask_to_in",
                "fslmaths",
                larmorx_nc_avwutils::fslmaths,
                args,
            )?;
            let masked = self.tmp(output, "tmp_masked");
            let args = cat(vec![inapply_words, one("-mas"), to_in, masked.clone()]);
            self.program(
                227,
                "${FSLDIR}/bin/fslmaths $inapply -mas ${out}_tmp_standard_mask_to_in ${out}_tmp_masked",
                "fslmaths",
                larmorx_nc_avwutils::fslmaths,
                args,
            )?;
            inapply_words = masked;
        }
        let outs = self.words(output, "the output");
        // then cut down FOV, if requested
        if roi < 3 {
            let dest = self.tmp(output, "tmp_standard_roi");
            if roi == 1 {
                fov(self, &std_words, dest.clone(), 234)?;
            } else {
                let rw = self.words(&roimask.clone().unwrap_or_default(), "the ROI mask");
                let args = cat(vec![
                    rw,
                    one("-bin"),
                    dest.clone(),
                    one("-odt"),
                    one("float"),
                ]);
                self.program(
                    236,
                    "${FSLDIR}/bin/fslmaths $roimask -bin ${out}_tmp_standard_roi -odt float",
                    "fslmaths",
                    larmorx_nc_avwutils::fslmaths,
                    args,
                )?;
            }
            let to_in = self.tmp(output, "tmp_standard_roi_to_in");
            let args = cat(vec![
                one("-ref"),
                ins.clone(),
                one("-in"),
                dest,
                one("-o"),
                to_in.clone(),
                one("-applyxfm"),
                one("-init"),
                inv,
            ]);
            self.program(
                238,
                "${FSLDIR}/bin/flirt -ref $in -in ${out}_tmp_standard_roi -o ${out}_tmp_standard_roi_to_in -applyxfm -init ${out}_tmp_to_std_inv.mat",
                "flirt",
                crate::flirt,
                args,
            )?;
            // fslroi $inapply $out `fslstats ${out}_tmp_standard_roi_to_in -w`
            let mut buf = Vec::new();
            let args = cat(vec![to_in, one("-w")]);
            let code = {
                let (fsl, mode, threads) = (self.fsl, self.mode, self.threads);
                let err = &mut *self.err;
                exception::catch(|| {
                    larmorx_nc_avwutils::fslstats(&args, fsl, mode, threads, &mut buf, err)
                })
                .unwrap_or(134)
            };
            self.check("fslstats", code)?;
            let box_words = split_words(&String::from_utf8_lossy(&buf));
            let args = cat(vec![inapply_words, outs, box_words]);
            self.program(
                239,
                "${FSLDIR}/bin/fslroi $inapply $out `${FSLDIR}/bin/fslstats ${out}_tmp_standard_roi_to_in -w`",
                "fslroi",
                larmorx_nc_avwutils::fslroi,
                args,
            )?;
        } else {
            // imcp $inapply $out: fslpy's imcp takes its two arguments as source and destination
            // (more sources need a directory as destination)
            let mut words = inapply_words;
            words.extend(outs);
            let code = if words.len() < 2
                || (words.len() > 2 && !std::path::Path::new(&words[words.len() - 1]).is_dir())
            {
                let _ = writeln!(self.out, "{}", fslpy::IMCP_USAGE);
                1
            } else {
                let dest = words.pop().unwrap_or_default();
                let mut code = 0;
                for src in &words {
                    match fslpy::copy(src, &dest, self.fsl, false) {
                        Ok(()) => {}
                        Err(f) if f.unsupported => {
                            // the replica's refusal (fslpy converts with nibabel): it stops
                            let _ = writeln!(self.err, "{PREFIX}: error: {NAME}: {}", f.text);
                            return Err(Stop::Exit(1));
                        }
                        Err(f) => {
                            if let Some(w) = &f.warning {
                                let _ = writeln!(self.err, "{w}");
                            }
                            let _ = writeln!(self.out, "{}", f.text);
                            code = 1;
                            break;
                        }
                    }
                }
                code
            };
            self.check("imcp", code)?;
        }
        Ok(())
    }

    /// `if [ $debug = 0 ]; then /bin/rm ${out}_tmp*; fi`: by default only this run's temporary
    /// files; the script's glob also removes any other file named so (FSL-212). Returns rm's
    /// exit status.
    fn cleanup(&mut self, output: &str) -> u8 {
        let words = if self.reproduce() {
            split_words(&format!("{output}_tmp*"))
        } else {
            vec![format!("{output}_tmp*")]
        };
        let mut targets: Vec<String> = Vec::new();
        for w in words {
            match w.strip_suffix('*') {
                Some(prefix) => targets.extend(avw::shell_glob_prefix(prefix)),
                None => targets.push(w),
            }
        }
        if !self.reproduce() {
            let ours: Vec<String> = targets
                .iter()
                .filter(|f| {
                    self.created
                        .iter()
                        .any(|c| *c == **f || avw::remove_ext(f) == *c)
                })
                .cloned()
                .collect();
            let others: Vec<&str> = targets
                .iter()
                .filter(|f| !ours.contains(f) && std::path::Path::new(f).exists())
                .map(String::as_str)
                .collect();
            if !others.is_empty() {
                self.notices.add(
                    &defects::FSL_212,
                    format!(
                        "kept files this run did not create (the script removes them): {}",
                        others.join(", ")
                    ),
                );
            }
            for f in ours {
                let _ = std::fs::remove_file(&f);
            }
            return 0;
        }
        let created = self.created.clone();
        let mut status = 0;
        let mut removed_others = Vec::new();
        for f in &targets {
            let p = std::path::Path::new(f);
            let meta = std::fs::symlink_metadata(p);
            match meta {
                Err(_) => {
                    let _ = writeln!(
                        self.err,
                        "/bin/rm: cannot remove '{f}': No such file or directory"
                    );
                    status = 1;
                }
                Ok(m) if m.is_dir() => {
                    let _ = writeln!(self.err, "/bin/rm: cannot remove '{f}': Is a directory");
                    status = 1;
                }
                Ok(_) => {
                    if !created.iter().any(|c| c == f || avw::remove_ext(f) == *c) {
                        removed_others.push(f.clone());
                    }
                    if std::fs::remove_file(p).is_err() {
                        let _ =
                            writeln!(self.err, "/bin/rm: cannot remove '{f}': Permission denied");
                        status = 1;
                    }
                }
            }
        }
        if !removed_others.is_empty() {
            self.notices.add(
                &defects::FSL_212,
                format!(
                    "removed files this run did not create, as the script does: {}",
                    removed_others.join(", ")
                ),
            );
        }
        status
    }
}
