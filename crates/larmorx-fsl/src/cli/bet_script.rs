// SPDX-License-Identifier: Apache-2.0
//! `bet -S`, `-B`, `-A`, `-A2` (`specs/bet.md` §5.3): each a fixed sequence of FSL's programs,
//! here the original's own run in this process (FLIRT, `standard_space_roi`, FAST, `fslmaths`,
//! `fslstats`, bet2, betsurf, and the bet script itself in `-B`) with the arguments the script
//! gives them; each program's messages, statuses and defects are its own.
//!
//! Built for speed (§10): every program is the original's (FLIRT, FAST and bet2 parallel
//! inside); the temporary images are written uncompressed when they are not kept (no `-d`, and
//! no `<out>_tmp` file before the run that a stale name could be confused with); the eye
//! threshold in exact decimal arithmetic. The programs run one after the other, as the script
//! runs them: two at once would hold both programs' images, and larmorx's peak memory is held to
//! FSL's, whose peak is its largest program's (CLAUDE.md, "Memory similar to the original's").
//!
//! The script's failure (bash's `ERR` trap): a program that exits with a status other than 0
//! (or that FSL aborts with signal 6, bash's `Aborted` report first) ends the script with
//! `bet failed during command:<the script's arguments>` on standard output and status 1, its
//! temporary files left (observed: the trap exits before the cleanup). The commands' texts in
//! bash's report are the script's own where an abort was observed (`fslmaths` of `-S`'s step 7,
//! bet2 of its step 3, betsurf of `-A2`), the others written in the same form.
//!
//! D13: FSL-216 (`-S`'s eye threshold read by dc from the printed numbers: the default reads them
//! as numbers and refuses one that is not a number; `reproduce` as dc reads a leading minus
//! sign), FSL-63 (`-B -s`'s removal of `-s` after bash's `echo`), FSL-44 (names with blanks,
//! `-A2`'s T2 among them, split into words with `reproduce`), FSL-46 (the cleanup, in
//! [`super::bet`]); the programs' own (FSL-214 in `standard_space_roi`, FSL-14 and FSL-68 in
//! FLIRT, ...).

use std::io::Write;
use std::path::PathBuf;

use larmorx_core::defects::{self, Notices, UpstreamDefects};
use larmorx_mri::hmc::files;

use super::Settings;
use super::common::ABORTED;

/// The line bash reports for every command of the script it saw killed by a signal (observed:
/// 399, the script's function that runs its commands).
const LINE: u32 = 399;

/// A word of a command as the script writes it: a fixed word, or a name or an option (expanded
/// unquoted by the script: split into words at blanks with `reproduce`, FSL-44), or text the
/// script always expands unquoted (a command substitution's output: split always).
pub enum W {
    L(&'static str),
    N(String),
    Words(String),
}

/// What happens to a program's standard output and error.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Streams {
    /// Printed as the program prints them.
    Through,
    /// Its standard output returned (a command substitution), its error printed.
    Capture,
    /// Both discarded but larmorx's own lines (`fast ... >/dev/null 2>&1`).
    Discard,
}

/// The script's state while it runs its programs.
pub struct Script<'a> {
    /// The run's settings: the outputs the script keeps.
    pub settings: &'a Settings,
    /// The temporary images' settings (`FSLOUTPUTTYPE` uncompressed when they are not kept).
    pub temp: Settings,
    pub mode: UpstreamDefects,
    /// The script's own arguments, for its failure line.
    pub args: &'a [String],
    /// The next process ID bash reports.
    pub pid: u32,
}

/// Whether a line is larmorx's own (a notice, a refusal): kept where FAST's streams are not.
fn larmorx_line(l: &str) -> bool {
    l.starts_with("larmorx: ") || l.starts_with("larmorx fsl ") || l.ends_with("(larmorx)")
}

impl Script<'_> {
    fn words(&self, parts: Vec<W>) -> Vec<String> {
        let split = |s: &str| -> Vec<String> { s.split_whitespace().map(str::to_owned).collect() };
        let mut out = Vec::new();
        for p in parts {
            match p {
                W::L(s) => out.push(s.to_owned()),
                W::N(s) if self.mode.reproduce() => out.extend(split(&s)),
                W::N(s) => out.push(s),
                W::Words(s) => out.extend(split(&s)),
            }
        }
        out
    }

    /// The script's failure line (its `ERR` trap) on standard output; the status 1.
    pub fn failed(&self, out: &mut dyn Write) -> u8 {
        let _ = writeln!(out, "bet failed during command:{}", self.args.join(" "));
        1
    }

    /// Runs the original's `tool` (`text`: the command as the script writes it, for bash's report
    /// of an abort; `last`: it writes the outputs the script keeps). The script fails where the
    /// program does.
    #[allow(clippy::too_many_arguments)]
    fn exec(
        &mut self,
        tool: &str,
        parts: Vec<W>,
        text: &str,
        last: bool,
        streams: Streams,
        notices: &mut Notices,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> Result<Vec<u8>, u8> {
        let args = self.words(parts);
        let settings = if last { self.settings } else { &self.temp };
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let code = if tool == "bet" {
            super::bet::script(&args, settings, notices, &mut o, &mut e)
        } else {
            super::run_with(tool, &args, settings, &mut o, &mut e).unwrap_or(127)
        };
        match streams {
            Streams::Through => {
                let _ = out.write_all(&o);
                let _ = err.write_all(&e);
            }
            Streams::Capture => {
                let _ = err.write_all(&e);
            }
            Streams::Discard => {
                for l in String::from_utf8_lossy(&o)
                    .lines()
                    .chain(String::from_utf8_lossy(&e).lines())
                {
                    if larmorx_line(l) {
                        let _ = writeln!(err, "{l}");
                    }
                }
            }
        }
        let aborted = String::from_utf8_lossy(&e).contains(ABORTED);
        if aborted {
            // bash's report of a program killed by signal 6 (observed)
            let pid = self.pid;
            self.pid += 1;
            let _ = writeln!(err, "bet: line {LINE}: {pid:>5} {:<24}{text}", "Aborted");
        }
        if code != 0 || aborted {
            return Err(self.failed(out));
        }
        Ok(o)
    }

    #[allow(clippy::too_many_arguments)]
    fn run(
        &mut self,
        tool: &str,
        parts: Vec<W>,
        text: &str,
        last: bool,
        notices: &mut Notices,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> Result<(), u8> {
        self.exec(tool, parts, text, last, Streams::Through, notices, out, err)
            .map(|_| ())
    }
}

/// The names of the script's temporary files.
fn tmp(output: &str, suffix: &str) -> W {
    W::N(format!("{output}_tmp{suffix}"))
}

fn names(v: &[String]) -> Vec<W> {
    v.iter().map(|s| W::N(s.clone())).collect()
}

/// `-S` (§5.3): the eyes and optic nerves removed. `eye_mask`: FSL's `MNI152_T1_2mm_eye_mask`
/// as found (§5.4).
#[allow(clippy::too_many_arguments)]
pub fn eyes(
    s: &mut Script<'_>,
    input: &str,
    o: &str,
    opts: &[String],
    eye_mask: &str,
    notices: &mut Notices,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), u8> {
    // 1. the raw brain and its skull
    let mut a = vec![W::N(input.to_owned()), tmp(o, "_betraw"), W::L("-s")];
    a.extend(names(opts));
    s.run(
        "bet2",
        a,
        "${FSLDIR}/bin/bet2 $IN ${OUT}_tmp_betraw -s $bet2opts",
        false,
        notices,
        out,
        err,
    )?;
    // 2. the input masked by MNI152's dilated brain mask (standard_space_roi keeps its files)
    s.run(
        "standard_space_roi",
        vec![
            W::N(input.to_owned()),
            tmp(o, "_premask"),
            W::L("-b"),
            W::L("-d"),
        ],
        "${FSLDIR}/bin/standard_space_roi $IN ${OUT}_tmp_premask -b -d",
        false,
        notices,
        out,
        err,
    )?;
    // 3. bet2 on it: only its mask is used (a -f among the options wins: bet2's last)
    let mut a = vec![
        tmp(o, "_premask"),
        tmp(o, "_premask_bet"),
        W::L("-n"),
        W::L("-m"),
        W::L("-f"),
        W::L("0.4"),
    ];
    a.extend(names(opts));
    s.run(
        "bet2",
        a,
        "${FSLDIR}/bin/bet2 ${OUT}_tmp_premask ${OUT}_tmp_premask_bet -n -m -f 0.4 $bet2opts",
        false,
        notices,
        out,
        err,
    )?;
    // 4. the eye mask on the input's grid
    s.run(
        "flirt",
        vec![
            W::L("-ref"),
            W::N(input.to_owned()),
            W::L("-in"),
            W::N(eye_mask.to_owned()),
            W::L("-applyxfm"),
            W::L("-init"),
            tmp(o, "_premask_tmp_to_std_inv.mat"),
            W::L("-datatype"),
            W::L("float"),
            W::L("-out"),
            tmp(o, "_eye_mask"),
        ],
        "${FSLDIR}/bin/flirt -ref $IN -in ${FSLDIR}/data/standard/MNI152_T1_2mm_eye_mask -applyxfm -init ${OUT}_tmp_premask_tmp_to_std_inv.mat -datatype float -out ${OUT}_tmp_eye_mask",
        false,
        notices,
        out,
        err,
    )?;
    // 5. thresholded, dilated once in 3D and seven times in 2D
    let mut a = vec![
        tmp(o, "_eye_mask"),
        W::L("-thr"),
        W::L("0.5"),
        W::L("-bin"),
        W::L("-dilF"),
        W::L("-kernel"),
        W::L("2D"),
    ];
    for _ in 0..7 {
        a.push(W::L("-dilF"));
    }
    a.extend([tmp(o, "_eyes7dil"), W::L("-odt"), W::L("short")]);
    s.run(
        "fslmaths",
        a,
        "${FSLDIR}/bin/fslmaths ${OUT}_tmp_eye_mask -thr 0.5 -bin -dilF -kernel 2D -dilF -dilF -dilF -dilF -dilF -dilF -dilF ${OUT}_tmp_eyes7dil -odt short",
        false,
        notices,
        out,
        err,
    )?;
    // 6. its inverse
    s.run(
        "fslmaths",
        vec![
            tmp(o, "_eyes7dil"),
            W::L("-mul"),
            W::L("-1"),
            W::L("-add"),
            W::L("1"),
            tmp(o, "_eyes7dil_inv"),
        ],
        "${FSLDIR}/bin/fslmaths ${OUT}_tmp_eyes7dil -mul -1 -add 1 ${OUT}_tmp_eyes7dil_inv",
        false,
        notices,
        out,
        err,
    )?;
    // 7. the raw brain outside the dilated eyes, or inside the premasked run's mask
    s.run(
        "fslmaths",
        vec![
            tmp(o, "_betraw"),
            W::L("-bin"),
            W::L("-mas"),
            tmp(o, "_eyes7dil_inv"),
            W::L("-add"),
            tmp(o, "_premask_bet_mask"),
            W::L("-bin"),
            W::L("-mul"),
            tmp(o, "_betraw"),
            tmp(o, "_prebrain"),
        ],
        "${FSLDIR}/bin/fslmaths ${OUT}_tmp_betraw -bin -mas ${OUT}_tmp_eyes7dil_inv -add ${OUT}_tmp_premask_bet_mask -bin -mul ${OUT}_tmp_betraw ${OUT}_tmp_prebrain",
        false,
        notices,
        out,
        err,
    )?;
    // 8. its mean and standard deviation, as fslstats prints them
    let mut stat = |s: &mut Script<'_>, opt: &'static str, text: &str| {
        s.exec(
            "fslstats",
            vec![tmp(o, "_prebrain"), W::L(opt)],
            text,
            false,
            Streams::Capture,
            notices,
            out,
            err,
        )
        .map(|o| {
            // a command substitution: the trailing newlines removed
            String::from_utf8_lossy(&o)
                .trim_end_matches('\n')
                .to_owned()
        })
    };
    let mean = stat(s, "-M", "${FSLDIR}/bin/fslstats ${OUT}_tmp_prebrain -M")?;
    let sd = stat(s, "-S", "${FSLDIR}/bin/fslstats ${OUT}_tmp_prebrain -S")?;
    // 9. the threshold, in dc's decimal arithmetic
    let thr = match eye_threshold(&mean, &sd, s.mode) {
        Ok(t) => {
            if let Some(d) = t.notice {
                notices.add(&defects::FSL_216, d);
            }
            if s.mode.reproduce() {
                let _ = err.write_all(t.dc_errors.as_bytes());
            }
            t.text
        }
        Err(why) => {
            notices.add(&defects::FSL_216, why.clone());
            let _ = writeln!(err, "larmorx fsl bet: {why} (larmorx)");
            return Err(s.failed(out));
        }
    };
    // 10. the brain below the threshold, opened, closed and eroded, times the raw brain
    let mut a = vec![
        tmp(o, "_prebrain"),
        W::L("-uthr"),
        W::Words(thr.clone()),
        W::L("-bin"),
        W::L("-ero"),
        W::L("-dilF"),
        W::L("-dilF"),
        W::L("-ero"),
        W::L("-mul"),
        tmp(o, "_prebrain"),
        W::L("-uthr"),
        W::Words(thr),
    ];
    a.extend([
        W::L("-bin"),
        W::L("-kernel"),
        W::L("2D"),
        W::L("-ero"),
        W::L("-dilF"),
        W::L("-mul"),
        tmp(o, "_betraw"),
        W::N(o.to_owned()),
    ]);
    s.run(
        "fslmaths",
        a,
        "${FSLDIR}/bin/fslmaths ${OUT}_tmp_prebrain -uthr $thr -bin -ero -dilF -dilF -ero -mul ${OUT}_tmp_prebrain -uthr $thr -bin -kernel 2D -ero -dilF -mul ${OUT}_tmp_betraw $OUT",
        true,
        notices,
        out,
        err,
    )?;
    // 11. the mask
    s.run(
        "fslmaths",
        vec![W::N(o.to_owned()), W::L("-bin"), W::N(format!("{o}_mask"))],
        "${FSLDIR}/bin/fslmaths $OUT -bin ${OUT}_mask",
        true,
        notices,
        out,
        err,
    )?;
    // 12. the skull without the eyes (dilated five times)
    s.run(
        "fslmaths",
        vec![
            tmp(o, "_eyes7dil"),
            W::L("-ero"),
            W::L("-kernel"),
            W::L("2D"),
            W::L("-ero"),
            W::L("-mul"),
            W::L("-1"),
            W::L("-add"),
            W::L("1"),
            tmp(o, "_eyes5dil_inv"),
        ],
        "${FSLDIR}/bin/fslmaths ${OUT}_tmp_eyes7dil -ero -kernel 2D -ero -mul -1 -add 1 ${OUT}_tmp_eyes5dil_inv",
        false,
        notices,
        out,
        err,
    )?;
    s.run(
        "fslmaths",
        vec![
            tmp(o, "_betraw_skull"),
            W::L("-mas"),
            tmp(o, "_eyes5dil_inv"),
            W::N(format!("{o}_skull")),
        ],
        "${FSLDIR}/bin/fslmaths ${OUT}_tmp_betraw_skull -mas ${OUT}_tmp_eyes5dil_inv ${OUT}_skull",
        true,
        notices,
        out,
        err,
    )
}

/// `-B` (§5.3): the bias field and the neck removed, then the robust run.
#[allow(clippy::too_many_arguments)]
pub fn bias(
    s: &mut Script<'_>,
    input: &str,
    o: &str,
    opts: &[String],
    notices: &mut Notices,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), u8> {
    // 1. the premask
    s.run(
        "standard_space_roi",
        vec![
            W::N(input.to_owned()),
            tmp(o, "_premask"),
            W::L("-b"),
            W::L("-d"),
        ],
        "${FSLDIR}/bin/standard_space_roi $IN ${OUT}_tmp_premask -b -d",
        false,
        notices,
        out,
        err,
    )?;
    // 2. FAST's bias field (its messages discarded, larmorx's kept)
    s.exec(
        "fast",
        vec![W::L("-b"), tmp(o, "_premask")],
        "${FSLDIR}/bin/fast -b ${OUT}_tmp_premask",
        false,
        Streams::Discard,
        notices,
        out,
        err,
    )?;
    // 3. the input divided by it
    s.run(
        "fslmaths",
        vec![
            W::N(input.to_owned()),
            W::L("-div"),
            tmp(o, "_premask_bias"),
            tmp(o, "_unbiased"),
        ],
        "${FSLDIR}/bin/fslmaths $IN -div ${OUT}_tmp_premask_bias ${OUT}_tmp_unbiased",
        false,
        notices,
        out,
        err,
    )?;
    let mut opts = opts.to_vec();
    // 4. the skull, from the unbiased image cut to the standard field of view
    if opts.iter().any(|w| w == "-s") {
        s.run(
            "standard_space_roi",
            vec![
                tmp(o, "_unbiased"),
                tmp(o, "_unbiased_forskull_reduced"),
                W::L("-maskFOV"),
                W::L("-roiNONE"),
                W::L("-d"),
            ],
            "${FSLDIR}/bin/standard_space_roi ${OUT}_tmp_unbiased ${OUT}_tmp_unbiased_forskull_reduced -maskFOV -roiNONE -d",
            false,
            notices,
            out,
            err,
        )?;
        let mut a = vec![
            tmp(o, "_unbiased_forskull_reduced"),
            tmp(o, "_unbiased_forskull"),
        ];
        a.extend(names(&opts));
        s.run(
            "bet2",
            a,
            "${FSLDIR}/bin/bet2 ${OUT}_tmp_unbiased_forskull_reduced ${OUT}_tmp_unbiased_forskull $bet2opts",
            false,
            notices,
            out,
            err,
        )?;
        // FSL-63: -s removed from the options' text after bash's echo
        let given: Vec<String> = opts.iter().filter(|w| *w != "-s").cloned().collect();
        let theirs = super::bet::echo_remove(&opts, "-s");
        if theirs != given {
            let detail = match s.mode {
                UpstreamDefects::Fix => format!(
                    "-B -s: the robust run gets the options without -s ({}); the script's echo and removal of the text -s make them {}",
                    given.join(" "),
                    theirs.join(" ")
                ),
                UpstreamDefects::Reproduce => format!(
                    "-B -s: the options pass through bash's echo and lose the text -s, as the script does: {}",
                    theirs.join(" ")
                ),
            };
            notices.add(&defects::FSL_63, detail);
        }
        opts = if s.mode.reproduce() { theirs } else { given };
        // fslpy's immv: the skull renamed
        let src = format!("{o}_tmp_unbiased_forskull_skull");
        if let Err(m) = move_image(&s.temp, &src, s.settings, &format!("{o}_skull")) {
            let _ = writeln!(out, "{m}");
            return Err(s.failed(out));
        }
    }
    // 5. the unbiased image premasked
    s.run(
        "standard_space_roi",
        vec![
            tmp(o, "_unbiased"),
            tmp(o, "_unbiased_premask"),
            W::L("-b"),
            W::L("-d"),
        ],
        "${FSLDIR}/bin/standard_space_roi ${OUT}_tmp_unbiased ${OUT}_tmp_unbiased_premask -b -d",
        false,
        notices,
        out,
        err,
    )?;
    // 6. the bet script again: the robust run, the brain always written
    let mut a = vec![tmp(o, "_unbiased_premask"), W::N(o.to_owned()), W::L("-m")];
    a.extend(names(&opts));
    a.extend([W::L("-R"), W::L("-d")]);
    s.run(
        "bet",
        a,
        "${FSLDIR}/bin/bet ${OUT}_tmp_unbiased_premask $OUT -m $bet2opts -R -d",
        true,
        notices,
        out,
        err,
    )
}

/// `-A` and `-A2` (§5.3): the skull and scalp surfaces (`t2`: `-A2`'s T2, as given).
#[allow(clippy::too_many_arguments)]
pub fn surfaces(
    s: &mut Script<'_>,
    input: &str,
    o: &str,
    opts: &[String],
    t2: Option<&str>,
    standard: &str,
    notices: &mut Notices,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), u8> {
    if let Some(t2) = t2 {
        s.run(
            "flirt",
            vec![
                W::L("-in"),
                W::N(t2.to_owned()),
                W::L("-ref"),
                W::N(input.to_owned()),
                W::L("-out"),
                tmp(o, "_T2_to_T1"),
                W::L("-omat"),
                tmp(o, "_T2_to_T1.mat"),
                W::L("-interp"),
                W::L("sinc"),
            ],
            "${FSLDIR}/bin/flirt -in $T2 -ref $IN -out ${OUT}_tmp_T2_to_T1 -omat ${OUT}_tmp_T2_to_T1.mat -interp sinc",
            false,
            notices,
            out,
            err,
        )?;
    }
    s.run(
        "flirt",
        vec![
            W::L("-ref"),
            W::N(standard.to_owned()),
            W::L("-in"),
            W::N(input.to_owned()),
            W::L("-omat"),
            tmp(o, "_T1_to_std.mat"),
        ],
        "${FSLDIR}/bin/flirt -ref ${FSLDIR}/data/standard/MNI152_T1_2mm -in $IN -omat ${OUT}_tmp_T1_to_std.mat",
        false,
        notices,
        out,
        err,
    )?;
    let mut a = vec![W::N(input.to_owned()), W::N(o.to_owned()), W::L("-e")];
    a.extend(names(opts));
    s.run(
        "bet2",
        a,
        "${FSLDIR}/bin/bet2 $IN $OUT -e $bet2opts",
        true,
        notices,
        out,
        err,
    )?;
    let mesh = W::N(format!("{o}_mesh.vtk"));
    let (a, text) = match t2 {
        None => (
            vec![
                W::L("--t1only"),
                W::L("-o"),
                W::L("-m"),
                W::L("-s"),
                W::N(input.to_owned()),
                mesh,
                tmp(o, "_T1_to_std.mat"),
                W::N(o.to_owned()),
            ],
            "${FSLDIR}/bin/betsurf --t1only -o -m -s $IN ${OUT}_mesh.vtk ${OUT}_tmp_T1_to_std.mat $OUT",
        ),
        Some(_) => (
            vec![
                W::L("-o"),
                W::L("-m"),
                W::L("-s"),
                W::N(input.to_owned()),
                tmp(o, "_T2_to_T1"),
                mesh,
                tmp(o, "_T1_to_std.mat"),
                W::N(o.to_owned()),
            ],
            // observed (FSL's abort on a T2 FLIRT could not read)
            "${FSLDIR}/bin/betsurf -o -m -s $IN ${OUT}_tmp_T2_to_T1 ${OUT}_mesh.vtk ${OUT}_tmp_T1_to_std.mat $OUT",
        ),
    };
    s.run("betsurf", a, text, true, notices, out, err)
}

/// fslpy's `immv <src> <dest>`: the image's files moved, converted to `to`'s `FSLOUTPUTTYPE`
/// when `from`'s writes another format (the temporary images uncompressed).
fn move_image(from: &Settings, src: &str, to: &Settings, dest: &str) -> Result<(), String> {
    let full = from.name(src);
    let resolved = files::resolve_input(&full)?;
    let f = files::ImageFiles::of(&resolved).ok_or_else(|| format!("{resolved}: not an image"))?;
    let same = from.get("FSLOUTPUTTYPE") == to.get("FSLOUTPUTTYPE");
    if same {
        let ext = files::image_extension(&resolved).unwrap_or("");
        let target = format!("{}{ext}", to.name(dest));
        let t = files::ImageFiles::of(&target).ok_or_else(|| format!("{target}: not an image"))?;
        std::fs::rename(&f.header, &t.header).map_err(|e| e.to_string())?;
        if let (Some(a), Some(b)) = (&f.data, &t.data) {
            std::fs::rename(a, b).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    super::ssroi::copy_image(to, &resolved, dest)?;
    let mut gone: Vec<PathBuf> = vec![f.header.clone()];
    gone.extend(f.data.clone());
    for p in gone {
        let _ = std::fs::remove_file(p);
    }
    Ok(())
}

// ------------------------------------------------------------------------------------------------
// The eye threshold (§5.3 step 9; FSL-216)

/// A decimal number as GNU dc holds one: its sign, the digits of its value times `10^scale`
/// (least significant first) and its scale (the digits after the point).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decimal {
    neg: bool,
    digits: Vec<u8>,
    scale: usize,
}

impl Decimal {
    fn trimmed(mut self) -> Decimal {
        while self.digits.len() > 1 && self.digits.last() == Some(&0) {
            self.digits.pop();
        }
        if self.digits.is_empty() {
            self.digits.push(0);
        }
        if self.is_zero() {
            self.neg = false;
        }
        self
    }

    fn is_zero(&self) -> bool {
        self.digits.iter().all(|&d| d == 0)
    }

    /// A number as a program prints it, `[-+]digits[.digits][e[-+]digits]`, its exponent written
    /// out exactly (`2.1e+06` is `2100000`, `1.23457e-05` is `.0000123457`); `None` for text that
    /// is not a number (`nan`, `inf`, anything else).
    pub fn parse(text: &str) -> Option<Decimal> {
        let t = text.trim();
        let (neg, t) = match t.as_bytes().first() {
            Some(b'-') => (true, &t[1..]),
            Some(b'+') => (false, &t[1..]),
            _ => (false, t),
        };
        let (mantissa, exp) = match t.find(['e', 'E']) {
            Some(k) => (&t[..k], t[k + 1..].parse::<i64>().ok()?),
            None => (t, 0),
        };
        let (int, frac) = mantissa.split_once('.').unwrap_or((mantissa, ""));
        if int.is_empty() && frac.is_empty()
            || !int.bytes().chain(frac.bytes()).all(|b| b.is_ascii_digit())
        {
            return None;
        }
        let mut digits: Vec<u8> = int.bytes().chain(frac.bytes()).map(|b| b - b'0').collect();
        digits.reverse();
        let mut scale = frac.len() as i64 - exp;
        if scale < 0 {
            let zeros = usize::try_from(-scale).ok()?;
            if zeros > 10_000 {
                return None;
            }
            let mut d = vec![0u8; zeros];
            d.extend(digits);
            digits = d;
            scale = 0;
        }
        Some(
            Decimal {
                neg,
                digits,
                scale: usize::try_from(scale).ok()?,
            }
            .trimmed(),
        )
    }

    /// The text dc prints: no leading zero before the point (`.4`), `-` for a negative value, the
    /// scale's digits kept (`10.0`); zero is `0`.
    pub fn dc_text(&self) -> String {
        if self.is_zero() {
            return "0".to_owned();
        }
        let mut d: Vec<u8> = self.digits.clone();
        while d.len() <= self.scale {
            d.push(0);
        }
        let s: String = d.iter().rev().map(|&x| char::from(b'0' + x)).collect();
        let (int, frac) = s.split_at(s.len() - self.scale);
        let int = int.trim_start_matches('0');
        let mut out = String::new();
        if self.neg {
            out.push('-');
        }
        out.push_str(int);
        if self.scale > 0 {
            out.push('.');
            out.push_str(frac);
        }
        out
    }

    /// The magnitude's digits at scale `scale` (`scale` at least the number's own).
    fn widened(&self, scale: usize) -> Vec<u8> {
        let mut d = vec![0u8; scale - self.scale];
        d.extend(&self.digits);
        d
    }
}

fn cmp_mag(a: &[u8], b: &[u8]) -> std::cmp::Ordering {
    let len = |v: &[u8]| v.iter().rposition(|&x| x != 0).map_or(0, |k| k + 1);
    let (la, lb) = (len(a), len(b));
    if la != lb {
        return la.cmp(&lb);
    }
    for k in (0..la).rev() {
        if a[k] != b[k] {
            return a[k].cmp(&b[k]);
        }
    }
    std::cmp::Ordering::Equal
}

fn add_mag(a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(a.len().max(b.len()) + 1);
    let mut carry = 0u8;
    for k in 0..a.len().max(b.len()) {
        let v = a.get(k).copied().unwrap_or(0) + b.get(k).copied().unwrap_or(0) + carry;
        out.push(v % 10);
        carry = v / 10;
    }
    if carry > 0 {
        out.push(carry);
    }
    out
}

/// `a - b` for `|a| >= |b|`.
fn sub_mag(a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(a.len());
    let mut borrow = 0i16;
    for (k, &x) in a.iter().enumerate() {
        let mut v = i16::from(x) - i16::from(b.get(k).copied().unwrap_or(0)) - borrow;
        borrow = 0;
        if v < 0 {
            v += 10;
            borrow = 1;
        }
        out.push(v as u8);
    }
    out
}

/// dc's `+`: exact, at the larger scale.
pub fn dc_add(a: &Decimal, b: &Decimal) -> Decimal {
    let scale = a.scale.max(b.scale);
    let (x, y) = (a.widened(scale), b.widened(scale));
    let (neg, digits) = if a.neg == b.neg {
        (a.neg, add_mag(&x, &y))
    } else if cmp_mag(&x, &y) != std::cmp::Ordering::Less {
        (a.neg, sub_mag(&x, &y))
    } else {
        (b.neg, sub_mag(&y, &x))
    };
    Decimal { neg, digits, scale }.trimmed()
}

/// dc's `*` at scale `k`: the exact product, truncated toward zero to
/// `min(a.scale + b.scale, max(k, a.scale, b.scale))` places.
pub fn dc_mul(a: &Decimal, b: &Decimal, k: usize) -> Decimal {
    let mut prod = vec![0u32; a.digits.len() + b.digits.len()];
    for (i, &x) in a.digits.iter().enumerate() {
        for (j, &y) in b.digits.iter().enumerate() {
            prod[i + j] += u32::from(x) * u32::from(y);
        }
    }
    let mut digits = Vec::with_capacity(prod.len() + 1);
    let mut carry = 0u32;
    for v in prod {
        let t = v + carry;
        digits.push((t % 10) as u8);
        carry = t / 10;
    }
    while carry > 0 {
        digits.push((carry % 10) as u8);
        carry /= 10;
    }
    let full = a.scale + b.scale;
    let scale = full.min(k.max(a.scale).max(b.scale));
    let drop = full - scale;
    let digits = if drop >= digits.len() {
        vec![0]
    } else {
        digits[drop..].to_vec()
    };
    Decimal {
        neg: a.neg != b.neg,
        digits,
        scale,
    }
    .trimmed()
}

/// `Mean + 3.5 × SD` as GNU dc computes it at scale 10, as its text.
pub fn dc_threshold(mean: &Decimal, sd: &Decimal) -> String {
    let k35 = Decimal {
        neg: false,
        digits: vec![5, 3],
        scale: 1,
    };
    dc_add(mean, &dc_mul(&k35, sd, 10)).dc_text()
}

/// The eye threshold's text, what dc prints on standard error with it, and the FSL-216 notice
/// when the default's result differs from FSL's.
pub struct Threshold {
    pub text: String,
    pub dc_errors: String,
    pub notice: Option<String>,
}

/// The eye threshold from fslstats' printed mean and deviation (§5.3 step 9; FSL-216). FSL hands
/// the text to dc as its input: a leading minus sign is dc's subtraction, with too few numbers on
/// its stack (`dc: stack empty` on standard error), so the number's magnitude is used; an
/// exponent's `e`, `nan` and `inf` are dc's commands. The default reads the numbers as numbers
/// (rule A) and refuses one that is not a number (rule B); `reproduce` reads a leading minus as
/// dc does, and refuses what it does not reproduce (an exponent, a word: dc's commands).
pub fn eye_threshold(mean: &str, sd: &str, mode: UpstreamDefects) -> Result<Threshold, String> {
    let (m, d) = match (Decimal::parse(mean), Decimal::parse(sd)) {
        (Some(m), Some(d)) => (m, d),
        _ => {
            return Err(format!(
                "-S: the brain's mean or standard deviation is not a number (fslstats printed '{}' and '{}'): the eye threshold cannot be computed; FSL's dc reads such text as its own commands (FSL-216)",
                mean.trim(),
                sd.trim()
            ));
        }
    };
    let exponent = |t: &str| t.contains(['e', 'E']);
    if mode.reproduce() && (exponent(mean) || exponent(sd)) {
        return Err(format!(
            "-S: fslstats printed a number with an exponent ('{}', '{}'), whose letters FSL's dc reads as its commands: not reproduced (FSL-216)",
            mean.trim(),
            sd.trim()
        ));
    }
    let ours = dc_threshold(&m, &d);
    // dc's reading of FSL's text: a leading minus is its subtraction (the magnitude kept)
    let dc_minus = |t: &str| t.trim_start().starts_with('-');
    let magnitude = |x: &Decimal| Decimal {
        neg: false,
        ..x.clone()
    };
    let (fm, fd) = (
        if dc_minus(mean) {
            magnitude(&m)
        } else {
            m.clone()
        },
        if dc_minus(sd) {
            magnitude(&d)
        } else {
            d.clone()
        },
    );
    let theirs = dc_threshold(&fm, &fd);
    let errors =
        "dc: stack empty\n".repeat(usize::from(dc_minus(mean)) + usize::from(dc_minus(sd)));
    let differs = ours != theirs || exponent(mean) || exponent(sd);
    let notice = differs.then(|| match mode {
        UpstreamDefects::Fix => format!(
            "-S's eye threshold Mean + 3.5 x SD read from the printed numbers ({}, {}): {ours}; FSL's dc reads their text as its commands and gives {}",
            mean.trim(),
            sd.trim(),
            if exponent(mean) || exponent(sd) {
                "another threshold".to_owned()
            } else {
                theirs.clone()
            }
        ),
        UpstreamDefects::Reproduce => format!(
            "-S's eye threshold as FSL's dc reads the printed numbers' text ({}, {}): {theirs}, where the numbers give {ours}",
            mean.trim(),
            sd.trim()
        ),
    });
    Ok(Threshold {
        text: if mode.reproduce() { theirs } else { ours },
        dc_errors: if mode.reproduce() {
            errors
        } else {
            String::new()
        },
        notice,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FSL's standard images a variation reads are found before anything runs (§5.3): an image
    /// found nowhere refuses with exit 1, naming it and the places looked, and writes nothing.
    #[test]
    fn standard_images_found_before_anything_runs() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        std::fs::write(p.join("in.nii.gz"), b"x").unwrap();
        let data = p.join("fsl data");
        std::fs::create_dir_all(data.join("standard")).unwrap();
        for n in ["MNI152_T1_2mm", "MNI152_T1_2mm_brain_mask_dil"] {
            std::fs::write(data.join("standard").join(format!("{n}.nii.gz")), b"x").unwrap();
        }
        let settings = Settings {
            vars: [
                ("FSLOUTPUTTYPE".to_owned(), "NIFTI_GZ".to_owned()),
                (
                    crate::data::DATA_ENV.to_owned(),
                    data.to_string_lossy().into_owned(),
                ),
            ]
            .into(),
            dir: Some(p.to_path_buf()),
            stdin: None,
            output_type_default: Default::default(),
        };
        let run = |w: &[&str]| {
            let args: Vec<String> = w.iter().map(|s| (*s).to_owned()).collect();
            let (mut o, mut e) = (Vec::new(), Vec::new());
            let c = super::super::bet::main(&args, &settings, &mut o, &mut e);
            (
                c,
                String::from_utf8(o).unwrap(),
                String::from_utf8(e).unwrap(),
            )
        };
        let written = || {
            let mut v: Vec<String> = std::fs::read_dir(p)
                .unwrap()
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.starts_with("out"))
                .collect();
            v.sort();
            v
        };
        // -S reads the eye mask too
        let (c, out, err) = run(&["in", "out", "-S", "-v"]);
        assert_eq!(c, 1);
        assert!(out.ends_with("variation=2\n"), "{out}");
        assert!(
            err.contains("MNI152_T1_2mm_eye_mask was not found")
                && err.contains("LARMORX_FSL_DATA=")
                && err.ends_with("(larmorx)\n"),
            "{err}"
        );
        assert!(written().is_empty(), "{:?}", written());
        // -B and -A find theirs and run: FLIRT or standard_space_roi then fails on the input,
        // which is no image, and the script with it
        for v in ["-B", "-A"] {
            let (c, out, _) = run(&["in", "out", v]);
            assert_eq!(c, 1);
            assert_eq!(
                out.lines().last(),
                Some(format!("bet failed during command:in out {v}").as_str()),
                "{out}"
            );
        }
    }

    fn dec(s: &str) -> Decimal {
        Decimal::parse(s).unwrap()
    }

    /// GNU dc 1.4.1's own prints of these expressions (black-box runs, 2026-11-05): `10 k <mean>
    /// 3.5 <sd> * + p`.
    #[test]
    fn dc_arithmetic() {
        for (mean, sd, want) in [
            ("523.418", "81.2345", "807.73875"),
            ("12.5", ".00000000001", "12.50000000003"),
            ("12.5", ".0000000001", "12.5000000003"),
            ("12.5", ".000000001", "12.5000000035"),
            ("1.000000", "2.000000", "8.0000000"),
            ("0.5", "0", ".5"),
            ("0.000000", "0.000000", "0"),
            ("0.000000", ".000001", ".0000035"),
            ("1.000000", "0.000000", "1.0000000"),
            ("100", "0", "100.0"),
            ("-5.000000", "1.000000", "-1.5000000"),
            ("-1.5", "1", "2.0"),
            (
                "12345678901234567890.123456",
                "98765432109876543210.654321",
                "358024691285802469127.4135795",
            ),
            ("1951833.117037", "463525.237060", "3574171.4467470"),
        ] {
            assert_eq!(dc_threshold(&dec(mean), &dec(sd)), want, "{mean} {sd}");
        }
        assert_eq!(dec("-.5").dc_text(), "-.5");
        assert_eq!(dec("2.1e+06").dc_text(), "2100000");
        assert_eq!(dec("1.23457e-05").dc_text(), ".0000123457");
        for bad in ["nan", "-nan", "inf", "", ".", "1.2.3", "e5"] {
            assert!(Decimal::parse(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn fsl_216() {
        // plain numbers: the same in both modes, nothing named
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let t = eye_threshold("523.418000 ", "81.234500 ", mode).unwrap();
            assert_eq!(t.text, "807.7387500");
            assert!(t.notice.is_none() && t.dc_errors.is_empty());
        }
        // a negative mean: dc's subtraction (`10 k -5.2 3.5 81.2345 * + p` prints
        // `dc: stack empty` and 289.52075, observed)
        let f = eye_threshold("-5.2", "81.2345", UpstreamDefects::Fix).unwrap();
        assert_eq!(f.text, "279.12075");
        assert!(f.notice.unwrap().contains("289.52075"));
        let r = eye_threshold("-5.2", "81.2345", UpstreamDefects::Reproduce).unwrap();
        assert_eq!(
            (r.text.as_str(), r.dc_errors.as_str()),
            ("289.52075", "dc: stack empty\n")
        );
        assert!(r.notice.is_some());
        // a negative zero: the same threshold, not named; dc's message in reproduce
        let z = eye_threshold("-0.000000", "1.000000", UpstreamDefects::Reproduce).unwrap();
        assert_eq!(z.text, "3.5000000");
        assert!(z.notice.is_none() && z.dc_errors == "dc: stack empty\n");
        // an exponent: read exactly by default (named), refused in reproduce
        let e = eye_threshold("2.1e+06", "1", UpstreamDefects::Fix).unwrap();
        assert_eq!(e.text, "2100003.5");
        assert!(e.notice.is_some());
        assert!(eye_threshold("2.1e+06", "1", UpstreamDefects::Reproduce).is_err());
        // not a number: refused in both modes (rule B)
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            assert!(eye_threshold("nan", "1", mode).is_err());
            assert!(eye_threshold("1", "-nan", mode).is_err());
            assert!(eye_threshold("inf", "1", mode).is_err());
        }
    }
}
