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
//! - `-S`, `-B`, `-A`, `-A2` need FSL's FLIRT (and FAST for `-B`): not yet replicated, refused
//!   with a message naming them.
//!
//! The script expands its arguments unquoted (FSL-44): the replica reproduces the word
//! splitting of blanks in `reproduce` mode and uses the names as given by default.

use std::io::Write;

use larmorx_core::defects::{self, Notices};

use crate::avw::{self, ToolFailure};
use crate::bet2::{self, Run};
use crate::fsl::volume::FslSettings;

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
}

/// `bet`'s main (`args[0]` is the program name).
pub fn main(args: &[String], run: &Run, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mut s = Script {
        run,
        out,
        err,
        notices: Notices::new(run.mode),
        args: args[1..].to_vec(),
        created: Vec::new(),
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
    notices.print(s.err);
    code
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

    fn fsl(&self) -> &FslSettings {
        self.run.fsl
    }

    /// Runs `bet2` with these words; a failure stops the script through its ERR trap.
    fn bet2(&mut self, words: &[String]) -> Result<(), Stop> {
        let mut argv = vec!["bet2".to_owned()];
        argv.extend(words.iter().cloned());
        let run = Run {
            fsl: self.run.fsl,
            mode: self.run.mode,
            threads: self.run.threads,
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
        let _ = in2;
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
        match variation {
            0 => self.bet2(&pair(&opts))?,
            1 => self.robust(&ins, &outs, &bet2opts)?,
            3 => return self.small_fov(&in_name, &out_name, &opts, debug),
            4 => self.fmri(&in_name, &out_name, &opts)?,
            2 | 5 | 6 | 7 => {
                let (what, need) = match variation {
                    2 => (
                        "-S (eye and optic nerve cleanup)",
                        "FLIRT (standard_space_roi and flirt)",
                    ),
                    5 => (
                        "-A (skull and scalp surfaces)",
                        "FLIRT (the registration to MNI152)",
                    ),
                    6 => (
                        "-A2 (skull and scalp surfaces with a T2)",
                        "FLIRT (the T2-to-T1 and MNI152 registrations)",
                    ),
                    _ => (
                        "-B (bias field and neck cleanup)",
                        "FLIRT (standard_space_roi) and FAST (the bias field)",
                    ),
                };
                let _ = writeln!(
                    self.err,
                    "bet: {what} runs FSL's {need}, which larmorx does not replicate yet; \
                     this option is pending (larmorx)"
                );
                return Err(Stop::Exit(1));
            }
            _ => {}
        }
        let _ = skull;
        self.cleanup(&out_name, debug);
        Ok(())
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

    /// `-R`.
    fn robust(&mut self, ins: &[String], outs: &[String], bet2opts: &str) -> Result<(), Stop> {
        // bet2opts=`echo $bet2opts | sed 's/-n//g'`
        let cleaned = split_words(bet2opts).join(" ").replace("-n", "");
        let opts = split_words(&cleaned);
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
        let line = self.tool(r)?;
        let w = split_words(&line);
        let g = |k: usize| w.get(k).cloned().unwrap_or_default();
        Ok((g(0), g(1), g(2)))
    }

    /// `-F`.
    fn fmri(&mut self, in_name: &str, out_name: &str, opts: &[String]) -> Result<(), Stop> {
        let tmp = format!("{out_name}_tmp");
        let r = avw::fslroi(in_name, &tmp, &["0", "1"], self.fsl());
        self.tool(r)?;
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
        );
        self.tool(r)?;
        let r = avw::fslmaths_mas(in_name, &format!("{out_name}_mask"), out_name, self.fsl());
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
            let r = avw::fslval(in_name, key);
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
        );
        self.tool(r)?;
        self.created(&bottom);
        let last = dc::eval(&format!("{zdim} 1 - p"), self.err);
        let r = avw::fslroi(
            in_name,
            &top,
            &["0", &xd, "0", &yd, last.trim(), "1"],
            self.fsl(),
        );
        self.tool(r)?;
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
        let r = avw::fslmerge_z(&padded, &inputs, self.fsl());
        self.tool(r)?;
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
        if orig_would_crop != ours {
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
            let r = avw::fslroi(&f, &f, &["0", &xd, "0", &yd, &npad_s, &zd], self.fsl());
            self.tool(r)?;
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

/// GNU dc for the expressions the script computes: numbers (`_` for a minus), `k`, `+ - * / ^`
/// and `p`, with bc's scale rules and truncation.
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

    fn parse(tok: &str) -> Option<Num> {
        let (neg, t) = match tok.strip_prefix('_') {
            Some(r) => (true, r),
            None => (false, tok),
        };
        let (ip, fp) = match t.split_once('.') {
            Some((a, b)) => (a, b),
            None => (t, ""),
        };
        if !(ip.chars().all(|c| c.is_ascii_digit()) && fp.chars().all(|c| c.is_ascii_digit())) {
            return None;
        }
        let digits = format!("{ip}{fp}");
        let mant: i128 = if digits.is_empty() {
            0
        } else {
            digits.parse().ok()?
        };
        Some(Num {
            mant: if neg { -mant } else { mant },
            scale: fp.len() as u32,
        })
    }

    fn show(a: Num) -> String {
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

    /// Evaluates `expr` and returns what `p` printed (dc's own messages to `err`).
    pub fn eval(expr: &str, err: &mut dyn Write) -> String {
        let mut stack: Vec<Num> = Vec::new();
        let mut k: u32 = 0;
        let mut out = String::new();
        for tok in expr.split_whitespace() {
            let op = |stack: &mut Vec<Num>, err: &mut dyn Write| -> Option<(Num, Num)> {
                if stack.len() < 2 {
                    let _ = writeln!(err, "dc: stack empty");
                    return None;
                }
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                Some((a, b))
            };
            match tok {
                "k" => match stack.pop() {
                    Some(v) => k = rescale(v, 0).mant.max(0) as u32,
                    None => {
                        let _ = writeln!(err, "dc: stack empty");
                    }
                },
                "p" => match stack.last() {
                    Some(v) => {
                        out.push_str(&show(*v));
                        out.push('\n');
                    }
                    None => {
                        let _ = writeln!(err, "dc: stack empty");
                    }
                },
                "+" | "-" => {
                    if let Some((a, b)) = op(&mut stack, err) {
                        let s = a.scale.max(b.scale);
                        let (a, b) = (rescale(a, s), rescale(b, s));
                        let mant = if tok == "+" {
                            a.mant + b.mant
                        } else {
                            a.mant - b.mant
                        };
                        stack.push(Num { mant, scale: s });
                    }
                }
                "*" => {
                    if let Some((a, b)) = op(&mut stack, err) {
                        let full = Num {
                            mant: a.mant * b.mant,
                            scale: a.scale + b.scale,
                        };
                        let s = (a.scale + b.scale).min(k.max(a.scale).max(b.scale));
                        stack.push(rescale(full, s));
                    }
                }
                "/" => {
                    if let Some((a, b)) = op(&mut stack, err) {
                        if b.mant == 0 {
                            let _ = writeln!(err, "dc: divide by zero");
                            stack.push(a);
                            stack.push(b);
                        } else {
                            // a / b at scale k, truncated: (a * 10^(k + sb - sa)) / b
                            let num = rescale(a, k + b.scale).mant;
                            stack.push(Num {
                                mant: num / b.mant,
                                scale: k,
                            });
                        }
                    }
                }
                "^" => {
                    if let Some((a, b)) = op(&mut stack, err) {
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
                            stack.push(rescale(full, s));
                        } else {
                            stack.push(Num { mant: 0, scale: k });
                        }
                    }
                }
                t => match parse(t) {
                    Some(v) => stack.push(v),
                    None => {
                        let _ = writeln!(err, "dc: '{t}' unimplemented");
                    }
                },
            }
        }
        out
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
    }
}
