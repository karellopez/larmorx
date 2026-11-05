// SPDX-License-Identifier: Apache-2.0
//! `larmorx fsl standard_space_roi`: FLIRT's `standard_space_roi` script
//! (`specs/standard_space_roi.md` §2): the input registered to a standard-space reference
//! (FSL's `MNI152_T1_2mm` unless `-ssref`), the reference's field of view or a standard-space
//! mask taken back into the input's space, the input masked with it and/or cut down to its
//! bounding box.
//!
//! The script's programs are the original's own, run in this process with the script's
//! arguments (FLIRT, `convert_xfm`, `fslmaths`, `fslstats`, `fslroi`, fslpy's `imcp`): their
//! messages and statuses are theirs. Built for speed (§7): the registration is almost all the
//! time; besides it, without `-d` the temporary images are written uncompressed (nobody reads
//! them after the run), and the field of view is resampled once when it is both the mask and
//! the region. FSL's standard images are found as `specs/bet.md` §5.4 says ([`crate::data`]).
//!
//! D13 (§6): FSL-211 (the names and options as given; `reproduce`: bash's word splitting with
//! its `[` messages), FSL-212 (the cleanup removes only this run's files), FSL-213 (`maskthr`
//! in the environment ignored), FSL-214 (the script stops at the first program that fails;
//! `reproduce`: it goes on, as FSL's).

use std::io::Write;
use std::path::{Path, PathBuf};

use larmorx_core::defects::{self, Notices, UpstreamDefects};
use larmorx_mri::hmc::files::{self, Compression};

use super::Settings;
use super::bet::remove_ext;
use crate::data::{self, StandardImage};

/// The script's name in bash's messages (the replica's and FSL's `$0` read as its name).
pub const SCRIPT: &str = "standard_space_roi";

/// The usage (§2.2), `${FSLDIR}` replaced by the variable's value.
pub fn usage(fsldir: &str) -> String {
    format!(
        "\nUsage: standard_space_roi <input> <output> [options] [flirt options]

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

/// The mask or the region: the reference's field of view, a standard-space image, FSL's dilated
/// brain mask (`-b`, found among FSL's standard images), or none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Choice {
    Fov,
    Image(String),
    Standard(StandardImage),
    None,
}

/// The command line, read (§2.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Call {
    pub input: String,
    pub output: String,
    /// The image masked and cut (`-altinput`), else the input.
    pub alt_input: Option<String>,
    /// `-ssref`'s image, else FSL's `MNI152_T1_2mm`.
    pub reference: Option<String>,
    pub mask: Choice,
    pub roi: Choice,
    /// `-b`: the transformed mask thresholded at 0.5 (kept when a later option changes the mask).
    pub betpremask: bool,
    pub debug: bool,
    /// The arguments from the first one that is not the script's: the first FLIRT run's.
    pub flirt: Vec<String>,
}

/// How reading the command line ends early: the exit status (messages printed).
pub struct Exit(pub u8);

/// A word of a command as the script writes it: a fixed word, a name (one word, split at blanks
/// by bash with `reproduce`: FSL-211) or words (always split: `$maskthr`, `fslstats`' output).
enum P {
    L(&'static str),
    N(String),
    W(String),
}

/// bash's `[ ... ]` (`test`, GNU bash 5.2) on `args`: its status (0 true, 1 false, 2 an error)
/// and its error message (`[: <message>`), for the script's tests of unquoted names (FSL-211).
pub fn bash_test(args: &[String]) -> (u8, Option<String>) {
    let mut t = Test { a: args, pos: 0 };
    let r = t.posix();
    match r {
        Err(m) => (2, Some(m)),
        Ok(v) => {
            if t.pos < args.len() {
                let tok = &args[t.pos];
                let m = if tok.starts_with('-') {
                    format!("syntax error: `{tok}' unexpected")
                } else {
                    "too many arguments".to_owned()
                };
                (2, Some(m))
            } else {
                (u8::from(!v), None)
            }
        }
    }
}

struct Test<'a> {
    a: &'a [String],
    pos: usize,
}

const BINARY: [&str; 14] = [
    "=", "==", "!=", "<", ">", "-eq", "-ne", "-lt", "-le", "-gt", "-ge", "-nt", "-ot", "-ef",
];

fn unary_op(s: &str) -> bool {
    s.len() == 2 && s.starts_with('-') && "abcdefghknoprstuwxzGLNOSRv".contains(&s[1..])
}

impl Test<'_> {
    fn posix(&mut self) -> Result<bool, String> {
        let n = self.a.len();
        let a = self.a;
        match n {
            0 => Ok(false),
            1 => {
                self.pos = 1;
                Ok(!a[0].is_empty())
            }
            2 => {
                self.pos = 2;
                self.two(0)
            }
            3 => self.three(),
            4 if a[0] == "!" => {
                self.pos = 1;
                self.three_at(1).map(|v| !v)
            }
            4 if a[0] == "(" && a[3] == ")" => {
                self.pos = 4;
                self.two(1)
            }
            _ => self.or(),
        }
    }

    fn two(&self, at: usize) -> Result<bool, String> {
        let (op, arg) = (&self.a[at], &self.a[at + 1]);
        if op == "!" {
            return Ok(arg.is_empty());
        }
        if unary_op(op) {
            return unary(op, arg);
        }
        Err(format!("{op}: unary operator expected"))
    }

    fn three(&mut self) -> Result<bool, String> {
        self.three_at(0)
    }

    fn three_at(&mut self, at: usize) -> Result<bool, String> {
        let a = self.a;
        self.pos = at + 3;
        if BINARY.contains(&a[at + 1].as_str()) {
            return binary(&a[at], &a[at + 1], &a[at + 2]);
        }
        if a[at + 1] == "-a" || a[at + 1] == "-o" {
            let (x, y) = (!a[at].is_empty(), !a[at + 2].is_empty());
            return Ok(if a[at + 1] == "-a" { x && y } else { x || y });
        }
        if a[at] == "!" {
            return self.two(at + 1).map(|v| !v);
        }
        if a[at] == "(" && a[at + 2] == ")" {
            return Ok(!a[at + 1].is_empty());
        }
        Err(format!("{}: binary operator expected", a[at + 1]))
    }

    fn or(&mut self) -> Result<bool, String> {
        let mut v = self.and()?;
        while self.pos < self.a.len() && self.a[self.pos] == "-o" {
            self.pos += 1;
            let w = self.and()?;
            v = v || w;
        }
        Ok(v)
    }

    fn and(&mut self) -> Result<bool, String> {
        let mut v = self.term()?;
        while self.pos < self.a.len() && self.a[self.pos] == "-a" {
            self.pos += 1;
            let w = self.term()?;
            v = v && w;
        }
        Ok(v)
    }

    fn term(&mut self) -> Result<bool, String> {
        let a = self.a;
        let n = a.len();
        if self.pos >= n {
            return Err(format!("argument expected after `{}'", a[n - 1]));
        }
        if a[self.pos] == "!" {
            self.pos += 1;
            return self.term().map(|v| !v);
        }
        if a[self.pos] == "(" {
            self.pos += 1;
            let v = self.or()?;
            if self.pos >= n || a[self.pos] != ")" {
                return Err("`)' expected".to_owned());
            }
            self.pos += 1;
            return Ok(v);
        }
        if self.pos + 3 <= n && BINARY.contains(&a[self.pos + 1].as_str()) {
            let v = binary(&a[self.pos], &a[self.pos + 1], &a[self.pos + 2])?;
            self.pos += 3;
            return Ok(v);
        }
        if self.pos + 2 <= n && unary_op(&a[self.pos]) {
            let v = unary(&a[self.pos], &a[self.pos + 1])?;
            self.pos += 2;
            return Ok(v);
        }
        let v = !a[self.pos].is_empty();
        self.pos += 1;
        Ok(v)
    }
}

fn unary(op: &str, arg: &str) -> Result<bool, String> {
    let p = Path::new(arg);
    Ok(match op {
        "-z" => arg.is_empty(),
        "-n" => !arg.is_empty(),
        "-e" | "-a" | "-r" | "-w" | "-x" => p.exists(),
        "-f" => p.is_file(),
        "-d" => p.is_dir(),
        "-s" => std::fs::metadata(p).is_ok_and(|m| m.len() > 0),
        "-h" | "-L" => p.is_symlink(),
        _ => false,
    })
}

fn binary(a: &str, op: &str, b: &str) -> Result<bool, String> {
    let int = |s: &str| -> Result<i64, String> {
        s.trim()
            .parse::<i64>()
            .map_err(|_| format!("{s}: integer expression expected"))
    };
    Ok(match op {
        "=" | "==" => a == b,
        "!=" => a != b,
        "<" => a < b,
        ">" => a > b,
        "-eq" => int(a)? == int(b)?,
        "-ne" => int(a)? != int(b)?,
        "-lt" => int(a)? < int(b)?,
        "-le" => int(a)? <= int(b)?,
        "-gt" => int(a)? > int(b)?,
        "-ge" => int(a)? >= int(b)?,
        _ => false,
    })
}

/// bash's splitting of an unquoted expansion (blanks, tabs and newlines; empty fields dropped).
fn split(s: &str) -> Vec<String> {
    s.split([' ', '\t', '\n'])
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

/// A run of the script: what its steps share.
struct Script<'a> {
    settings: &'a Settings,
    /// The settings of the temporary images: `FSLOUTPUTTYPE`'s uncompressed form when they are
    /// removed at the end (no `-d`), else the run's own.
    temp: Settings,
    mode: UpstreamDefects,
    /// The next process ID bash's reports name.
    pid: u32,
    /// Every step so far exited 0 and printed nothing but larmorx's own lines: the steps that
    /// repeat an earlier one's command on other names would print nothing either (§7's shared
    /// field of view is taken only then).
    clean: bool,
}

/// Whether `bytes` hold a line that is not larmorx's own (a notice, a refusal).
fn has_own_lines(bytes: &[u8]) -> bool {
    String::from_utf8_lossy(bytes).lines().any(|l| {
        !(l.starts_with("larmorx: ") || l.starts_with("larmorx fsl ") || l.ends_with("(larmorx)"))
    })
}

/// A step's end: go on, or stop the script with this status (FSL-214's correction).
type Step = Result<Vec<u8>, u8>;

impl Script<'_> {
    fn words(&self, parts: Vec<P>) -> Vec<String> {
        let mut out = Vec::new();
        for p in parts {
            match p {
                P::L(s) => out.push(s.to_owned()),
                P::N(s) if self.mode.reproduce() => out.extend(split(&s)),
                P::N(s) => out.push(s),
                P::W(s) => out.extend(split(&s)),
            }
        }
        out
    }

    /// Runs the original's `tool` with `args` (script line `line`, the command as the script
    /// writes it for bash's report of an abort). `capture`: its standard output returned
    /// instead of printed (a command substitution). `last`: the output's own step (the run's
    /// `FSLOUTPUTTYPE`).
    #[allow(clippy::too_many_arguments)]
    fn run(
        &mut self,
        tool: &str,
        args: Vec<P>,
        line: u32,
        text: &str,
        capture: bool,
        last: bool,
        notices: &mut Notices,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> Step {
        let args = self.words(args);
        let settings = if last { self.settings } else { &self.temp };
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let code = super::run_with(tool, &args, settings, &mut o, &mut e).unwrap_or(127);
        if !capture {
            let _ = out.write_all(&o);
        }
        let _ = err.write_all(&e);
        let aborted = String::from_utf8_lossy(&e).contains(super::common::ABORTED);
        if code != 0 || aborted || has_own_lines(&e) || (!capture && has_own_lines(&o)) {
            self.clean = false;
        }
        if aborted {
            // bash's report of a program killed by signal 6 (observed)
            let pid = self.pid;
            self.pid += 1;
            let _ = writeln!(
                err,
                "{SCRIPT}: line {line}: {pid:>5} {:<24}{text}",
                "Aborted"
            );
        }
        if code != 0 && self.mode.fix() {
            notices.add(
                &defects::FSL_214,
                format!(
                    "{tool} failed (exit status {}): the script stops here with exit status 1; FSL's goes on with the next steps on what is missing",
                    if aborted { 134 } else { code }
                ),
            );
            return Err(1);
        }
        Ok(o)
    }
}

/// Runs the script.
pub fn main(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mode = match settings.defects() {
        Ok(m) => m,
        Err(e) => {
            let _ = writeln!(err, "larmorx fsl standard_space_roi: {e}");
            return defects::EXIT_BAD_MODE;
        }
    };
    let mut notices = Notices::new(mode);
    let code = run(args, settings, &mut notices, out, err);
    notices.print(err);
    code
}

/// The script's `[` messages (FSL-211, reproduced): `<script>: line <n>: [: <message>`.
fn test_message(err: &mut dyn Write, line: u32, m: Option<String>) {
    if let Some(m) = m {
        let _ = writeln!(err, "{SCRIPT}: line {line}: [: {m}");
    }
}

/// `remove_ext` of a name (with `reproduce`, of each of its words, joined by blanks; an empty
/// name gives remove_ext's usage: FSL-211).
fn name_of(s: &str, mode: UpstreamDefects) -> String {
    if mode.reproduce() {
        let w = split(s);
        if w.is_empty() {
            return "Usage: remove_ext <list of image paths to remove extension from>".to_owned();
        }
        return w
            .iter()
            .map(|x| remove_ext(x).to_owned())
            .collect::<Vec<_>>()
            .join(" ");
    }
    remove_ext(s).to_owned()
}

/// Whether `name` is an image (FSL's `imtest`): a file of that name or of one of FSL's
/// completions exists. With `reproduce` a name of several words is not one (observed).
fn imtest(settings: &Settings, name: &str, mode: UpstreamDefects) -> bool {
    if name.is_empty() || (mode.reproduce() && split(name).len() != 1) {
        return false;
    }
    let full = settings.name(name);
    let stem = files::strip_image_extension(&full);
    files::COMPLETIONS
        .iter()
        .any(|e| Path::new(&format!("{stem}{e}")).is_file())
        || (files::image_extension(&full).is_some() && Path::new(&full).is_file())
}

/// Reads the command line (§2.1). `Err`: the run ends with that status, its message printed.
pub fn parse(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<Call, Exit> {
    let fsldir = settings.get("FSLDIR").unwrap_or("");
    let usage = |out: &mut dyn Write| {
        let _ = out.write_all(usage(fsldir).as_bytes());
        Exit(1)
    };
    let second = args.get(1).map_or("", String::as_str);
    let show_usage = if mode.reproduce() {
        // the script's test of its second argument, unquoted (observed at line 112)
        let mut t = split(&format!("X{second}"));
        t.extend(["=".to_owned(), "X".to_owned()]);
        let (status, m) = bash_test(&t);
        test_message(err, 112, m);
        status == 0
    } else {
        second.is_empty()
    };
    if show_usage {
        return Err(usage(out));
    }
    let input = name_of(&args[0], mode);
    if !imtest(settings, &input, mode) {
        let _ = writeln!(out, "Input image {input} not valid");
        return Err(Exit(1));
    }
    let mut call = Call {
        output: name_of(second, mode),
        input,
        alt_input: None,
        reference: None,
        mask: Choice::Fov,
        roi: Choice::Fov,
        betpremask: false,
        debug: false,
        flirt: Vec::new(),
    };
    let mut i = 2;
    while i < args.len() {
        let w = &args[i];
        // the options' loop: `[ _$1 != _ ]` (observed at line 133)
        if mode.reproduce() {
            let mut t = split(&format!("_{w}"));
            t.extend(["!=".to_owned(), "_".to_owned()]);
            let (status, m) = bash_test(&t);
            test_message(err, 133, m);
            if status != 0 {
                break;
            }
        } else if w.is_empty() {
            break;
        }
        let value = |i: usize| -> Option<&String> { args.get(i + 1).filter(|v| !v.is_empty()) };
        match w.as_str() {
            "-d" => call.debug = true,
            "-b" => {
                call.betpremask = true;
                call.mask = Choice::Standard(data::MNI152_T1_2MM_BRAIN_MASK_DIL);
                call.roi = Choice::None;
            }
            "-maskFOV" => call.mask = Choice::Fov,
            "-maskNONE" => call.mask = Choice::None,
            "-roiFOV" => call.roi = Choice::Fov,
            "-roiNONE" => call.roi = Choice::None,
            "-ssref" | "-altinput" | "-maskMASK" | "-roiMASK" => {
                let Some(v) = value(i) else {
                    return Err(usage(out));
                };
                if !imtest(settings, v, mode) {
                    match w.as_str() {
                        "-ssref" => {
                            let _ = writeln!(out, "standard space reference image not valid");
                        }
                        "-altinput" => {
                            let _ = writeln!(out, "Input image {v} not valid");
                        }
                        _ => {
                            let _ = writeln!(out, "Image {v} not valid");
                        }
                    }
                    return Err(Exit(1));
                }
                let v = v.clone();
                match w.as_str() {
                    "-ssref" => call.reference = Some(v),
                    "-altinput" => call.alt_input = Some(v),
                    "-maskMASK" => call.mask = Choice::Image(v),
                    _ => call.roi = Choice::Image(v),
                }
                i += 1;
            }
            _ => break,
        }
        i += 1;
    }
    call.flirt = args[i.min(args.len())..].to_vec();
    Ok(call)
}

/// `FSLOUTPUTTYPE`'s uncompressed form (`NIFTI_GZ` → `NIFTI`, `NIFTI2_PAIR_ZST` →
/// `NIFTI2_PAIR`): `None` when it is unset or unknown (the programs print FSL's messages).
pub(super) fn uncompressed(t: Option<&str>) -> Option<String> {
    let t = t?;
    if !crate::bet::image::OUTPUT_TYPES.contains(&t) {
        return None;
    }
    Some(
        ["_GZ", "_ZST", "_BZ2"]
            .iter()
            .find_map(|s| t.strip_suffix(s))
            .unwrap_or(t)
            .to_owned(),
    )
}

fn run(
    args: &[String],
    settings: &Settings,
    notices: &mut Notices,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mode = notices.mode();
    if mode.reproduce() && args.iter().any(|a| a.contains(['*', '?', '['])) {
        let _ = writeln!(
            err,
            "larmorx fsl standard_space_roi: an argument holds a shell pattern character (*, ? or [): the script expands its arguments unquoted, so the shell would expand it as a file-name pattern, which is not reproduced (FSL-211) (larmorx)"
        );
        return 1;
    }
    let call = match parse(args, settings, mode, out, err) {
        Ok(c) => c,
        Err(Exit(c)) => return c,
    };
    // FSL-211, named once the names are used (a refusal before changes nothing)
    if args.iter().any(|a| a.contains([' ', '\t', '\n']))
        || args.first().is_some_and(String::is_empty)
    {
        let detail = match mode {
            UpstreamDefects::Fix => {
                "the names and options are used as given; the script splits names with blanks into words, and an empty input becomes remove_ext's usage"
            }
            UpstreamDefects::Reproduce => {
                "the names and options expanded unquoted, as the script does: names with blanks split into words, with the shell's messages"
            }
        };
        notices.add(&defects::FSL_211, detail);
    }

    // FSL's standard images the call reads, found before anything runs (§4)
    let mut want: Vec<StandardImage> = Vec::new();
    if call.reference.is_none() {
        want.push(data::MNI152_T1_2MM);
    }
    if let Choice::Standard(i) = &call.mask {
        want.push(*i);
    }
    let mut found = Vec::new();
    for image in want {
        match data::find(settings, &data::FSL, &image, err) {
            Ok(f) => found.push((image.name, f.stem)),
            Err(m) => {
                let _ = writeln!(err, "larmorx fsl standard_space_roi: {m} (larmorx)");
                return 1;
            }
        }
    }
    let standard_of = |name: &str| {
        found
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, s)| s.clone())
            .unwrap_or_default()
    };
    let standard = call
        .reference
        .clone()
        .unwrap_or_else(|| standard_of(data::MNI152_T1_2MM.name));
    let mask = match &call.mask {
        Choice::Standard(i) => Choice::Image(standard_of(i.name)),
        other => other.clone(),
    };
    // FSL-213: maskthr from the environment (the script sets it only for -b)
    let env_thr = settings.get("maskthr").filter(|v| !v.is_empty());
    let maskthr = if call.betpremask {
        "-thr 0.5".to_owned()
    } else if let Some(v) = env_thr {
        let detail = match mode {
            UpstreamDefects::Fix => format!(
                "the variable maskthr ({v}) in the environment is ignored; the script applies it to the transformed mask as fslmaths options"
            ),
            UpstreamDefects::Reproduce => format!(
                "the variable maskthr ({v}) in the environment applied to the transformed mask, as the script does"
            ),
        };
        if call.mask != Choice::None {
            notices.add(&defects::FSL_213, detail);
        }
        if mode.reproduce() {
            v.to_owned()
        } else {
            String::new()
        }
    } else {
        String::new()
    };
    let before = snapshot(settings, &call.output, mode);
    let mut temp = settings.clone();
    if !call.debug
        && let Some(t) = uncompressed(settings.get("FSLOUTPUTTYPE"))
    {
        temp.vars.insert("FSLOUTPUTTYPE".into(), t);
    }
    let mut script = Script {
        settings,
        temp,
        mode,
        pid: std::process::id(),
        clean: true,
    };
    let result = steps(
        &mut script,
        &call,
        &standard,
        &mask,
        &maskthr,
        notices,
        out,
        err,
    );
    let code = match result {
        Ok(()) => 0,
        Err(c) => c,
    };
    if call.debug {
        // FSL's script ends with its cleanup's test, whose status is 0 with -d: it checks no
        // step (§2.3; the default stops at a failure with status 1, FSL-214)
        return if mode.reproduce() { 0 } else { code };
    }
    let rm = cleanup(settings, &call.output, &before, mode, notices, err);
    if mode.reproduce() { rm } else { code }
}

/// The steps 1 to 5 of §2.3.
#[allow(clippy::too_many_arguments)]
fn steps(
    s: &mut Script<'_>,
    call: &Call,
    standard: &str,
    mask: &Choice,
    maskthr: &str,
    notices: &mut Notices,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), u8> {
    let o = &call.output;
    let tmp = |x: &str| P::N(format!("{o}_tmp{x}"));
    let input = &call.input;
    // 1. the registration to the reference
    let mut a = vec![
        P::L("-in"),
        P::N(input.clone()),
        P::L("-ref"),
        P::N(standard.to_owned()),
        P::L("-o"),
        tmp("_to_std"),
        P::L("-omat"),
        tmp("_to_std.mat"),
    ];
    a.extend(call.flirt.iter().map(|w| P::N(w.clone())));
    s.run(
        "flirt",
        a,
        211,
        "${FSLDIR}/bin/flirt -in $in -ref $standard -o ${out}_tmp_to_std -omat ${out}_tmp_to_std.mat $@",
        false,
        false,
        notices,
        out,
        err,
    )?;
    // 2. its inverse
    s.run(
        "convert_xfm",
        vec![
            P::L("-inverse"),
            P::L("-omat"),
            tmp("_to_std_inv.mat"),
            tmp("_to_std.mat"),
        ],
        212,
        "${FSLDIR}/bin/convert_xfm -inverse -omat ${out}_tmp_to_std_inv.mat ${out}_tmp_to_std.mat",
        false,
        false,
        notices,
        out,
        err,
    )?;
    // 3. the reference's dimensions less 2 (fslval)
    let dims = reference_dims(s.settings, standard, s.mode);
    let [x, y, z] = dims.map(|d| (d - 2).to_string());
    let fov = |name: &str| -> Vec<P> {
        vec![
            P::N(standard.to_owned()),
            P::L("-mul"),
            P::L("0"),
            P::L("-add"),
            P::L("1"),
            P::L("-roi"),
            P::L("1"),
            P::W(x.clone()),
            P::L("1"),
            P::W(y.clone()),
            P::L("1"),
            P::W(z.clone()),
            P::L("0"),
            P::L("1"),
            P::N(format!("{o}_tmp{name}")),
            P::L("-odt"),
            P::L("float"),
        ]
    };
    let mut apply = call.alt_input.clone().unwrap_or_else(|| input.clone());
    // 4. the mask
    if *mask != Choice::None {
        let (args, text) = match mask {
            Choice::Image(m) => (
                vec![
                    P::N(m.clone()),
                    P::L("-bin"),
                    tmp("_standard_mask"),
                    P::L("-odt"),
                    P::L("float"),
                ],
                "${FSLDIR}/bin/fslmaths $maskmask -bin ${out}_tmp_standard_mask -odt float",
            ),
            _ => (
                fov("_standard_mask"),
                "${FSLDIR}/bin/fslmaths $standard -mul 0 -add 1 -roi 1 $X 1 $Y 1 $Z 0 1 ${out}_tmp_standard_mask -odt float",
            ),
        };
        s.run("fslmaths", args, 229, text, false, false, notices, out, err)?;
        s.run(
            "flirt",
            vec![
                P::L("-ref"),
                P::N(input.clone()),
                P::L("-in"),
                tmp("_standard_mask"),
                P::L("-o"),
                tmp("_standard_mask_to_in"),
                P::L("-applyxfm"),
                P::L("-init"),
                tmp("_to_std_inv.mat"),
            ],
            229,
            "${FSLDIR}/bin/flirt -ref $in -in ${out}_tmp_standard_mask -o ${out}_tmp_standard_mask_to_in -applyxfm -init ${out}_tmp_to_std_inv.mat",
            false,
            false,
            notices,
            out,
            err,
        )?;
        s.run(
            "fslmaths",
            vec![
                tmp("_standard_mask_to_in"),
                P::W(maskthr.to_owned()),
                tmp("_standard_mask_to_in"),
            ],
            229,
            "${FSLDIR}/bin/fslmaths ${out}_tmp_standard_mask_to_in $maskthr ${out}_tmp_standard_mask_to_in",
            false,
            false,
            notices,
            out,
            err,
        )?;
        s.run(
            "fslmaths",
            vec![
                P::N(apply.clone()),
                P::L("-mas"),
                tmp("_standard_mask_to_in"),
                tmp("_masked"),
            ],
            229,
            "${FSLDIR}/bin/fslmaths $inapply -mas ${out}_tmp_standard_mask_to_in ${out}_tmp_masked",
            false,
            false,
            notices,
            out,
            err,
        )?;
        apply = format!("{o}_tmp_masked");
    }
    // 5. the cut
    if call.roi == Choice::None {
        return imcp(s, &apply, o, notices, out, err);
    }
    // the field of view as both the mask and the region, and its files not kept: the mask's
    // resampling is the region's (§7; the threshold left it unchanged)
    let shared = call.roi == Choice::Fov
        && *mask == Choice::Fov
        && !call.debug
        && maskthr.is_empty()
        && s.clean;
    let region = if shared {
        "_standard_mask_to_in"
    } else {
        let (args, text) = match &call.roi {
            Choice::Image(m) => (
                vec![
                    P::N(m.clone()),
                    P::L("-bin"),
                    tmp("_standard_roi"),
                    P::L("-odt"),
                    P::L("float"),
                ],
                "${FSLDIR}/bin/fslmaths $roimask -bin ${out}_tmp_standard_roi -odt float",
            ),
            _ => (
                fov("_standard_roi"),
                "${FSLDIR}/bin/fslmaths $standard -mul 0 -add 1 -roi 1 $X 1 $Y 1 $Z 0 1 ${out}_tmp_standard_roi -odt float",
            ),
        };
        s.run("fslmaths", args, 242, text, false, false, notices, out, err)?;
        s.run(
            "flirt",
            vec![
                P::L("-ref"),
                P::N(input.clone()),
                P::L("-in"),
                tmp("_standard_roi"),
                P::L("-o"),
                tmp("_standard_roi_to_in"),
                P::L("-applyxfm"),
                P::L("-init"),
                tmp("_to_std_inv.mat"),
            ],
            242,
            "${FSLDIR}/bin/flirt -ref $in -in ${out}_tmp_standard_roi -o ${out}_tmp_standard_roi_to_in -applyxfm -init ${out}_tmp_to_std_inv.mat",
            false,
            false,
            notices,
            out,
            err,
        )?;
        "_standard_roi_to_in"
    };
    let text = "${FSLDIR}/bin/fslroi $inapply $out `${FSLDIR}/bin/fslstats ${out}_tmp_standard_roi_to_in -w`";
    let w = s.run(
        "fslstats",
        vec![tmp(region), P::L("-w")],
        242,
        text,
        true,
        false,
        notices,
        out,
        err,
    )?;
    let words = String::from_utf8_lossy(&w).into_owned();
    s.run(
        "fslroi",
        vec![P::N(apply), P::N(o.clone()), P::W(words)],
        242,
        text,
        false,
        true,
        notices,
        out,
        err,
    )?;
    Ok(())
}

/// The reference's `dim1`, `dim2`, `dim3` as `fslval` prints them (its header's).
fn reference_dims(settings: &Settings, name: &str, mode: UpstreamDefects) -> [i64; 3] {
    let files = crate::cli::bet2::resolve_image(settings, name, mode, &mut Vec::new()).ok();
    files
        .and_then(|f| larmorx_io::nifti::read_header(&f.header).ok())
        .map_or([0, 0, 0], |h| [1, 2, 3].map(|k| h.dim[k]))
}

/// fslpy's `imcp <src> <dest>` (§2.5): the source's files copied when they are in
/// `FSLOUTPUTTYPE`'s format, else converted (the same header and data, in the output's file
/// layout and compression).
fn imcp(
    s: &mut Script<'_>,
    src: &str,
    dest: &str,
    notices: &mut Notices,
    out: &mut dyn Write,
    _err: &mut dyn Write,
) -> Result<(), u8> {
    let settings = s.settings;
    // a source that is no image: fslpy's message (observed, 2026-11-05: bet -S of an image with
    // NaN voxels, whose masking step FSL's script went past)
    let r = match source_files(settings, src) {
        Err(_) => Err(format!(
            "Could not find a supported file with prefix \"{src}\""
        )),
        Ok(_) => copy_image(settings, src, dest),
    };
    match r {
        Ok(()) => Ok(()),
        Err(m) => {
            // fslpy prints the exception's text on standard output and exits 1
            let _ = writeln!(out, "{m}");
            if s.mode.fix() {
                notices.add(
                    &defects::FSL_214,
                    "imcp failed (exit status 1): the script stops here with exit status 1",
                );
            }
            Err(1)
        }
    }
}

/// The files of the image `src` (a name without extension completed as FSL completes it).
fn source_files(settings: &Settings, src: &str) -> Result<files::ImageFiles, String> {
    let full = settings.name(src);
    let resolved = files::resolve_input(&full)?;
    files::ImageFiles::of(&resolved).ok_or_else(|| format!("{resolved}: not an image"))
}

/// Copies or converts the image `src` to `dest` in `FSLOUTPUTTYPE`'s format.
pub fn copy_image(settings: &Settings, src: &str, dest: &str) -> Result<(), String> {
    let f = source_files(settings, src)?;
    let t = settings
        .get("FSLOUTPUTTYPE")
        .filter(|t| crate::bet::image::OUTPUT_TYPES.contains(t))
        .unwrap_or("NIFTI_GZ");
    let stem = settings.name(files::strip_image_extension(dest));
    let (name, _) = crate::bet::image::output_file(&stem, t)
        .ok_or_else(|| format!("{t}: not an output type"))?;
    let target = files::ImageFiles::of(&name).ok_or_else(|| format!("{name}: not an image"))?;
    let src_ext = files::image_extension(&f.header.to_string_lossy()).unwrap_or("");
    let dst_ext = files::image_extension(&name).unwrap_or("");
    if src_ext == dst_ext {
        std::fs::copy(&f.header, &target.header).map_err(|e| e.to_string())?;
        if let (Some(a), Some(b)) = (&f.data, &target.data) {
            std::fs::copy(a, b).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    // another format: the content in the output's layout (nibabel's conversion)
    let read = |p: &Path| -> Result<Vec<u8>, String> {
        let raw = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
        let c = Compression::of_bytes(&raw).unwrap_or(Compression::None);
        c.decompress(raw)
            .map_err(|e| format!("{}: {e}", p.display()))
    };
    let head = read(&f.header)?;
    let (mut header, data) = match &f.data {
        Some(d) => {
            let size = header_size(&head);
            (head[..size.min(head.len())].to_vec(), read(d)?)
        }
        None => {
            let size = header_size(&head);
            let offset = vox_offset(&head).max(size);
            (
                head[..offset.min(head.len())].to_vec(),
                head[offset.min(head.len())..].to_vec(),
            )
        }
    };
    let v2 = header_size(&header) == 540;
    let write = |p: &Path, pieces: &[&[u8]]| -> Result<(), String> {
        let c = Compression::of_name(&p.to_string_lossy());
        let file = std::fs::File::create(p).map_err(|e| format!("{}: {e}", p.display()))?;
        c.write_pieces(pieces, std::io::BufWriter::new(file))
            .map_err(|e| format!("{}: {e}", p.display()))
    };
    match &target.data {
        None => {
            // a single file: magic n+1 / n+2, the data after the header and its extension flag
            let size = header_size(&header);
            header.truncate(size);
            set_magic(&mut header, v2, true);
            let offset = size + 4;
            set_vox_offset(&mut header, v2, offset as f64);
            let flags = [0u8; 4];
            write(&target.header, &[&header, &flags, &data])
        }
        Some(img) => {
            let size = header_size(&header);
            header.truncate(size);
            set_magic(&mut header, v2, false);
            set_vox_offset(&mut header, v2, 0.0);
            write(&target.header, &[&header])?;
            write(img, &[&data])
        }
    }
}

fn header_size(h: &[u8]) -> usize {
    match h.get(..4) {
        Some(b) if u32::from_le_bytes([b[0], b[1], b[2], b[3]]) == 540 => 540,
        Some(b) if u32::from_be_bytes([b[0], b[1], b[2], b[3]]) == 540 => 540,
        _ => 348,
    }
}

fn little(h: &[u8]) -> bool {
    h.get(..4)
        .is_none_or(|b| matches!(u32::from_le_bytes([b[0], b[1], b[2], b[3]]), 348 | 540))
}

fn vox_offset(h: &[u8]) -> usize {
    if h.len() < 348 {
        return h.len();
    }
    if header_size(h) == 540 {
        let b: [u8; 8] = h[168..176].try_into().unwrap_or([0; 8]);
        let v = if little(h) {
            i64::from_le_bytes(b)
        } else {
            i64::from_be_bytes(b)
        };
        return usize::try_from(v).unwrap_or(540);
    }
    let b: [u8; 4] = h[108..112].try_into().unwrap_or([0; 4]);
    let v = if little(h) {
        f32::from_le_bytes(b)
    } else {
        f32::from_be_bytes(b)
    };
    if v.is_finite() && v >= 0.0 {
        v as usize
    } else {
        348
    }
}

fn set_vox_offset(h: &mut [u8], v2: bool, v: f64) {
    let le = little(h);
    if v2 {
        let b = if le {
            (v as i64).to_le_bytes()
        } else {
            (v as i64).to_be_bytes()
        };
        h[168..176].copy_from_slice(&b);
    } else if h.len() >= 112 {
        let b = if le {
            (v as f32).to_le_bytes()
        } else {
            (v as f32).to_be_bytes()
        };
        h[108..112].copy_from_slice(&b);
    }
}

fn set_magic(h: &mut [u8], v2: bool, single: bool) {
    if v2 {
        let m: &[u8; 8] = if single {
            b"n+2\0\r\n\x1a\n"
        } else {
            b"ni2\0\r\n\x1a\n"
        };
        h[4..12].copy_from_slice(m);
    } else if h.len() >= 348 && &h[344..347] != b"\0\0\0" {
        // NIfTI-1 (an ANALYZE header has no magic: kept so)
        h[344..348].copy_from_slice(if single { b"n+1\0" } else { b"ni1\0" });
    }
}

/// The files whose names start with `<output>_tmp` in the output's directory, with their sizes
/// and modification times (what the run did not create or change is not its own).
type Snapshot = Vec<(PathBuf, u64, Option<std::time::SystemTime>)>;

fn tmp_files(settings: &Settings, output: &str) -> Vec<PathBuf> {
    let full = settings.path(&format!("{output}_tmp"));
    let dir = full
        .parent()
        .map(Path::to_path_buf)
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or_else(|| PathBuf::from("."));
    let prefix = full
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut found: Vec<_> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with(&prefix))
        .map(|e| e.path())
        .collect();
    found.sort();
    found
}

fn snapshot(settings: &Settings, output: &str, mode: UpstreamDefects) -> Snapshot {
    if mode.reproduce() {
        return Vec::new();
    }
    tmp_files(settings, output)
        .into_iter()
        .map(|p| {
            let m = std::fs::metadata(&p).ok();
            let len = m.as_ref().map_or(0, std::fs::Metadata::len);
            let t = m.and_then(|m| m.modified().ok());
            (p, len, t)
        })
        .collect()
}

/// The cleanup (§2.3 step 6). The default removes the files this run wrote (FSL-212: the
/// others named); `reproduce` runs FSL's `rm ${out}_tmp*`: every file named so, with rm's
/// messages; its status is the script's.
fn cleanup(
    settings: &Settings,
    output: &str,
    before: &Snapshot,
    mode: UpstreamDefects,
    notices: &mut Notices,
    err: &mut dyn Write,
) -> u8 {
    if mode.fix() {
        let mut others = Vec::new();
        for p in tmp_files(settings, output) {
            let m = std::fs::metadata(&p).ok();
            let now = (
                m.as_ref().map_or(0, std::fs::Metadata::len),
                m.and_then(|m| m.modified().ok()),
            );
            let kept = before
                .iter()
                .any(|(q, len, t)| *q == p && (*len, *t) == now);
            if kept {
                others.push(
                    p.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                );
            } else if p.is_file() {
                let _ = std::fs::remove_file(&p);
            }
        }
        if !others.is_empty() {
            notices.add(
                &defects::FSL_212,
                format!(
                    "the cleanup removes only the run's own temporary files; the script also removes {}",
                    others.join(", ")
                ),
            );
        }
        return 0;
    }
    // bash: rm ${out}_tmp* (the words of the name, the last one's pattern expanded)
    let mut words = split(&format!("{output}_tmp*"));
    let pattern = words.pop().unwrap_or_default();
    let expanded = expand(settings, &pattern);
    if expanded.is_empty() {
        words.push(pattern);
    } else {
        words.extend(expanded);
    }
    let mut status = 0;
    for w in words {
        let p = settings.path(&w);
        let m = if p.is_dir() {
            Some("Is a directory")
        } else if std::fs::remove_file(&p).is_err() {
            Some("No such file or directory")
        } else {
            None
        };
        if let Some(m) = m {
            let _ = writeln!(err, "/bin/rm: cannot remove '{w}': {m}");
            status = 1;
        }
    }
    status
}

/// bash's expansion of a word ending in `*` (its other characters plain): the matching names,
/// sorted (C locale), with the word's directory as written.
fn expand(settings: &Settings, pattern: &str) -> Vec<String> {
    let Some(prefix) = pattern.strip_suffix('*') else {
        return Vec::new();
    };
    let (dir, start) = match prefix.rfind('/') {
        Some(k) => (&prefix[..=k], &prefix[k + 1..]),
        None => ("", prefix),
    };
    let d = if dir.is_empty() {
        settings.path(".")
    } else {
        settings.path(dir)
    };
    let mut names: Vec<String> = std::fs::read_dir(d)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(start) && !(n.starts_with('.') && !start.starts_with('.')))
        .collect();
    names.sort();
    names.into_iter().map(|n| format!("{dir}{n}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &[&str]) -> Vec<String> {
        w.iter().map(|x| (*x).to_owned()).collect()
    }

    #[test]
    fn bash_tests() {
        // the observed messages (FSL 6.0.7.23's script under GNU bash 5.2)
        assert_eq!(
            bash_test(&s(&["Xout", "put", "=", "X"])),
            (2, Some("too many arguments".into()))
        );
        assert_eq!(
            bash_test(&s(&["_-d", "-maskNONE", "!=", "_"])),
            (2, Some("syntax error: `-maskNONE' unexpected".into()))
        );
        assert_eq!(bash_test(&s(&["X", "=", "X"])), (0, None));
        assert_eq!(bash_test(&s(&["Xout", "=", "X"])), (1, None));
        assert_eq!(bash_test(&s(&["_", "!=", "_"])), (1, None));
        assert_eq!(bash_test(&s(&["_a", "-a", "b", "!=", "_"])), (0, None));
        assert_eq!(
            bash_test(&s(&["_a", "b", "c", "!=", "_"])),
            (2, Some("too many arguments".into()))
        );
    }

    #[test]
    fn output_types_uncompressed() {
        assert_eq!(uncompressed(Some("NIFTI_GZ")).as_deref(), Some("NIFTI"));
        assert_eq!(
            uncompressed(Some("NIFTI2_PAIR_ZST")).as_deref(),
            Some("NIFTI2_PAIR")
        );
        assert_eq!(uncompressed(Some("NIFTI")).as_deref(), Some("NIFTI"));
        assert_eq!(uncompressed(Some("ANALYZE")), None);
        assert_eq!(uncompressed(None), None);
    }

    #[test]
    fn the_command_line() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        for n in ["in.nii.gz", "ref.nii", "m.nii.gz"] {
            std::fs::write(p.join(n), b"x").unwrap();
        }
        let settings = Settings {
            vars: [("FSLDIR".to_owned(), "/opt/fsl".to_owned())].into(),
            dir: Some(p.to_path_buf()),
            stdin: None,
        };
        let run = |w: &[&str]| {
            let (mut o, mut e) = (Vec::new(), Vec::new());
            let r = parse(&s(w), &settings, UpstreamDefects::Fix, &mut o, &mut e);
            (r, String::from_utf8(o).unwrap())
        };
        let (r, o) = run(&["in"]);
        assert!(matches!(r, Err(Exit(1))));
        assert!(o.contains("(default /opt/fsl/data/standard/MNI152_T1)"));
        let (r, o) = run(&["nothere", "out"]);
        assert!(matches!(r, Err(Exit(1))));
        assert_eq!(o, "Input image nothere not valid\n");
        // -b then -maskMASK: the last mask, -b's threshold kept, no cut
        let (r, _) = run(&[
            "in.nii.gz",
            "out.nii.gz",
            "-b",
            "-maskMASK",
            "m",
            "-dof",
            "6",
        ]);
        let c = r.ok().unwrap();
        assert_eq!((c.input.as_str(), c.output.as_str()), ("in", "out"));
        assert!(c.betpremask);
        assert_eq!(c.mask, Choice::Image("m".into()));
        assert_eq!(c.roi, Choice::None);
        assert_eq!(c.flirt, s(&["-dof", "6"]));
        // the last of each kind; an empty argument ends the options
        let (r, _) = run(&["in", "out", "-maskNONE", "-maskFOV", "-roiNONE", "", "-d"]);
        let c = r.ok().unwrap();
        assert_eq!((c.mask, c.roi), (Choice::Fov, Choice::None));
        assert!(!c.debug);
        assert_eq!(c.flirt, s(&["", "-d"]));
        // a value missing: the usage; an invalid image: its message
        let (r, o) = run(&["in", "out", "-ssref"]);
        assert!(matches!(r, Err(Exit(1))) && o.starts_with("\nUsage:"));
        let (_, o) = run(&["in", "out", "-ssref", "nope"]);
        assert_eq!(o, "standard space reference image not valid\n");
        let (_, o) = run(&["in", "out", "-roiMASK", "nope"]);
        assert_eq!(o, "Image nope not valid\n");
        let (_, o) = run(&["in", "out", "-altinput", "nope"]);
        assert_eq!(o, "Input image nope not valid\n");
    }

    #[test]
    fn imcp_converts_the_layout() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        // a NIfTI-1 single file: header, extension flags, data
        let mut h = vec![0u8; 348];
        h[..4].copy_from_slice(&348u32.to_le_bytes());
        h[108..112].copy_from_slice(&352f32.to_le_bytes());
        h[344..348].copy_from_slice(b"n+1\0");
        let data = [1u8, 2, 3, 4];
        let mut file = h.clone();
        file.extend_from_slice(&[0, 0, 0, 0]);
        file.extend_from_slice(&data);
        std::fs::write(p.join("a.nii"), &file).unwrap();
        let mut settings = Settings {
            vars: [("FSLOUTPUTTYPE".to_owned(), "NIFTI_PAIR".to_owned())].into(),
            dir: Some(p.to_path_buf()),
            stdin: None,
        };
        copy_image(&settings, "a", "b").unwrap();
        let hdr = std::fs::read(p.join("b.hdr")).unwrap();
        assert_eq!(hdr.len(), 348);
        assert_eq!(&hdr[344..348], b"ni1\0");
        assert_eq!(std::fs::read(p.join("b.img")).unwrap(), data);
        // back to a single gzipped file: the same bytes as the original file
        settings
            .vars
            .insert("FSLOUTPUTTYPE".into(), "NIFTI_GZ".into());
        copy_image(&settings, "b", "c").unwrap();
        let raw = std::fs::read(p.join("c.nii.gz")).unwrap();
        let back = Compression::Gzip.decompress(raw).unwrap();
        assert_eq!(back, file);
        // the same format: a copy
        settings.vars.insert("FSLOUTPUTTYPE".into(), "NIFTI".into());
        copy_image(&settings, "a", "d").unwrap();
        assert_eq!(std::fs::read(p.join("d.nii")).unwrap(), file);
    }
}
