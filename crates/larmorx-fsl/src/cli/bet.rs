// SPDX-License-Identifier: Apache-2.0
//! `larmorx fsl bet`: the `bet` script's command line and variations (`specs/bet.md` §5).
//!
//! The script runs bet2 (here [`super::bet2`], in this process) with the options it is given,
//! or one of its variations: `-R` (bet2 repeated from the brain's centre), `-F` (4D data), `-Z`
//! (a small field of view in z), and `-S`, `-B`, `-A`, `-A2`, which run FSL's other programs
//! (the original's own, [`super::bet_script`]) and read FSL's standard images
//! ([`crate::data`]). Its messages are FSL's, its own path printed as `bet`.

use std::io::Write;
use std::path::{Path, PathBuf};

use larmorx_core::defects::{self, Notices, UpstreamDefects};
use larmorx_mri::hmc::files::{self, ImageFiles};

use super::Settings;
use super::bet_script::{self, Script};
use super::bet2;
use crate::bet::Vol;
use crate::bet::image::{self, Output, Values};
use crate::data;

/// bet's usage, exactly (standard output).
pub const USAGE: &str = "\nUsage:    bet <input> <output> [options]\n\nMain bet2 options:\n  -o          generate brain surface outline overlaid onto original image\n  -m          generate binary brain mask\n  -s          generate approximate skull image\n  -n          don't generate segmented brain image output\n  -f <f>      fractional intensity threshold (0->1); default=0.5; smaller values give larger brain outline estimates\n  -g <g>      vertical gradient in fractional intensity threshold (-1->1); default=0; positive values give larger brain outline at bottom, smaller at top\n  -r <r>      head radius (mm not voxels); initial surface sphere is set to half of this\n  -c <x y z>  centre-of-gravity (voxels not mm) of initial mesh surface.\n  -t          apply thresholding to segmented brain image and mask\n  -e          generates brain surface as mesh in .vtk format\n\nVariations on default bet2 functionality (mutually exclusive options):\n  (default)   just run bet2\n  -R          robust brain centre estimation (iterates BET several times)\n  -S          eye & optic nerve cleanup (can be useful in SIENA - disables -o option)\n  -B          bias field & neck cleanup (can be useful in SIENA)\n  -Z          improve BET if FOV is very small in Z (by temporarily padding end slices)\n  -F          apply to 4D FMRI data (uses -f 0.3 and dilates brain mask slightly)\n  -A          run bet2 and then betsurf to get additional skull and scalp surfaces (includes registrations)\n  -A2 <T2>    as with -A, when also feeding in non-brain-extracted T2 (includes registrations)\n\nMiscellaneous options:\n  -v          verbose (switch on diagnostic messages)\n  -h          display this help, then exits\n  -d          debug (don't delete temporary intermediate images)\n\n";

/// The variations, by the number `-v` prints (observed).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variation {
    Default = 0,
    Robust = 1,
    Eyes = 2,
    SmallFov = 3,
    Fmri = 4,
    Surfaces = 5,
    SurfacesT2 = 6,
    Bias = 7,
}

/// fslpy's `remove_ext`: the first of FSL's extensions that ends the name, removed.
pub fn remove_ext(name: &str) -> &str {
    for e in [
        ".nii.gz", ".nii", ".hdr.gz", ".hdr", ".img.gz", ".img", ".mnc", ".mnc.gz",
    ] {
        if let Some(s) = name.strip_suffix(e) {
            return s;
        }
    }
    name
}

struct Run<'a> {
    settings: &'a Settings,
    mode: UpstreamDefects,
    /// The script's own arguments, for its failure line.
    args: &'a [String],
}

impl Run<'_> {
    /// bet2 with `args`; on failure the script's line and exit 1.
    fn bet2(&self, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Result<(), u8> {
        // FSL-44 reproduced: the script's unquoted expansion of bet2's command line
        let split: Vec<String>;
        let args = if self.mode.reproduce() {
            split = args
                .iter()
                .flat_map(|a| a.split_whitespace().map(str::to_owned))
                .collect();
            &split[..]
        } else {
            args
        };
        let code = bet2::main(args, self.settings, out, err);
        if code == 0 {
            Ok(())
        } else {
            Err(self.failed(out))
        }
    }

    fn failed(&self, out: &mut dyn Write) -> u8 {
        let _ = writeln!(out, "bet failed during command:{}", self.args.join(" "));
        1
    }
}

/// Runs the `bet` script.
pub fn main(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mode = match settings.defects() {
        Ok(m) => m,
        Err(e) => {
            let _ = writeln!(err, "larmorx fsl bet: {e} (larmorx)");
            return 1;
        }
    };
    let mut notices = Notices::new(mode);
    let code = script(args, settings, &mut notices, out, err);
    notices.print(err);
    code
}

/// The script itself, its notices gathered in `notices` (`-B` runs the script again, sharing
/// them).
pub(super) fn script(
    args: &[String],
    settings: &Settings,
    notices: &mut Notices,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mode = notices.mode();
    // FSL-44: the script expands its names and options unquoted where it runs its programs (a
    // name with a blank becomes several words), and an empty argument ends its options
    if args
        .iter()
        .any(|a| a.contains(char::is_whitespace) || a.is_empty())
    {
        let detail = match mode {
            UpstreamDefects::Fix => {
                "the arguments are used as given; the script splits names with blanks into words and stops its options at an empty argument"
            }
            UpstreamDefects::Reproduce => {
                "the arguments are expanded unquoted, as the script expands them: names with blanks split into words, the options ending at an empty argument"
            }
        };
        notices.add(&defects::FSL_44, detail);
    }
    let words: Vec<String> = args.to_vec();
    if words.len() < 2 || words[1].is_empty() {
        let _ = out.write_all(USAGE.as_bytes());
        return 1;
    }
    let input = remove_ext(&words[0]).to_owned();
    let output = remove_ext(&words[1]).to_owned();
    let blank = mode.reproduce() && input.contains(char::is_whitespace);
    if blank || image::resolve_input(&settings.name(&input)).is_err() {
        let _ = write!(out, "\nError: input image {input} not valid\n\n");
        return 1;
    }
    let mut opts: Vec<String> = Vec::new();
    let mut variation = Variation::Default;
    let mut verbose = false;
    let mut debug = false;
    let mut t2: Option<String> = None;
    let mut i = 2;
    while i < words.len() {
        let w = words[i].as_str();
        if w.is_empty() {
            if mode.reproduce() {
                break;
            }
            i += 1;
            continue;
        }
        match w {
            "-o" | "-m" | "-s" | "-n" | "-t" | "-e" => opts.push(w.to_owned()),
            "-f" | "-g" | "-r" => {
                if i + 1 >= words.len() {
                    let _ = out.write_all(USAGE.as_bytes());
                    return 1;
                }
                opts.push(w.to_owned());
                opts.push(words[i + 1].clone());
                i += 1;
            }
            "-c" => {
                if i + 3 >= words.len() {
                    let _ = out.write_all(USAGE.as_bytes());
                    return 1;
                }
                opts.extend(words[i..=i + 3].iter().cloned());
                i += 3;
            }
            "-R" => variation = Variation::Robust,
            "-S" => variation = Variation::Eyes,
            "-Z" => variation = Variation::SmallFov,
            "-F" => variation = Variation::Fmri,
            "-A" => variation = Variation::Surfaces,
            "-B" => variation = Variation::Bias,
            "-A2" => {
                if i + 1 >= words.len() {
                    let _ = out.write_all(USAGE.as_bytes());
                    return 1;
                }
                variation = Variation::SurfacesT2;
                t2 = Some(words[i + 1].clone());
                i += 1;
            }
            "-v" => {
                verbose = true;
                opts.push("-v".into());
            }
            // larmorx's own (GLIBC-06, the user's decision of 2026-10-31): passed to bet2
            "--tanh" => {
                if i + 1 >= words.len() {
                    let _ = out.write_all(USAGE.as_bytes());
                    return 1;
                }
                opts.push(format!("--tanh={}", words[i + 1]));
                i += 1;
            }
            w if w.starts_with("--tanh=") => opts.push(w.to_owned()),
            "-d" => debug = true,
            _ => {
                let _ = out.write_all(USAGE.as_bytes());
                return 1;
            }
        }
        i += 1;
    }
    if verbose {
        let _ = writeln!(out, "IN={input}");
        let _ = writeln!(out, "OUT={output}");
        let shown: String = opts.iter().map(|o| format!(" {o}")).collect();
        let _ = writeln!(out, "bet2opts={shown}");
        let _ = writeln!(out, "verbose=1");
        let _ = writeln!(out, "debug={}", u8::from(debug));
        let _ = writeln!(out, "variation={}", variation as u8);
    }
    // FSL's standard images the variation reads, found before anything runs (§5.3, §5.4)
    let want: &[data::StandardImage] = match variation {
        Variation::Eyes => &[
            data::MNI152_T1_2MM,
            data::MNI152_T1_2MM_BRAIN_MASK_DIL,
            data::MNI152_T1_2MM_EYE_MASK,
        ],
        Variation::Bias => &[data::MNI152_T1_2MM, data::MNI152_T1_2MM_BRAIN_MASK_DIL],
        Variation::Surfaces | Variation::SurfacesT2 => &[data::MNI152_T1_2MM],
        _ => &[],
    };
    let mut standard = Vec::new();
    for image in want {
        match data::find(settings, &data::FSL, image, err) {
            Ok(f) => standard.push(f.stem),
            Err(m) => {
                let _ = writeln!(err, "larmorx fsl bet: {m} (larmorx)");
                return 1;
            }
        }
    }
    let run = Run {
        settings,
        mode,
        args,
    };
    let before = snapshot(settings, &output);
    let result = match variation {
        Variation::Robust => robust(&run, &input, &output, &opts, notices, out, err),
        Variation::Fmri => fmri(&run, &input, &output, &opts, debug, notices, out, err),
        Variation::SmallFov => small_fov(&run, &input, &output, &opts, debug, notices, out, err),
        Variation::Eyes | Variation::Bias | Variation::Surfaces | Variation::SurfacesT2 => {
            // the temporary images uncompressed when nobody reads them after the run (and no
            // earlier file of their names could be read instead)
            let mut temp = settings.clone();
            let mut light = false;
            if !debug
                && before.is_empty()
                && let Some(t) = super::ssroi::uncompressed(settings.get("FSLOUTPUTTYPE"))
                && settings.get("FSLOUTPUTTYPE") != Some(t.as_str())
            {
                temp.vars.insert("FSLOUTPUTTYPE".into(), t);
                light = true;
            }
            let mut s = Script {
                settings,
                temp,
                mode,
                args,
                pid: std::process::id(),
            };
            let r = match variation {
                Variation::Eyes => bet_script::eyes(
                    &mut s,
                    &input,
                    &output,
                    &opts,
                    &standard[2],
                    notices,
                    out,
                    err,
                ),
                Variation::Bias => {
                    bet_script::bias(&mut s, &input, &output, &opts, notices, out, err)
                }
                _ => bet_script::surfaces(
                    &mut s,
                    &input,
                    &output,
                    &opts,
                    t2.as_deref(),
                    &standard[0],
                    notices,
                    out,
                    err,
                ),
            };
            if r.is_err() && light {
                // the script leaves its temporary files when it fails: in the run's format, as
                // FSL writes them
                keep_temporaries(settings, &output);
            }
            r
        }
        Variation::Default => {
            let mut a = vec![input.clone(), output.clone()];
            a.extend(opts.iter().cloned());
            run.bet2(&a, out, err)
        }
    };
    // a step that failed ends the script before its cleanup (bash's ERR trap: observed, its
    // temporary files left)
    if !debug && result.is_ok() {
        cleanup(settings, &output, &before, mode, notices);
    }
    match result {
        Ok(()) => 0,
        Err(code) => code,
    }
}

/// The temporary images of a failed run, written uncompressed, rewritten in `FSLOUTPUTTYPE`'s
/// format (the failure leaves them, as FSL's script does).
fn keep_temporaries(settings: &Settings, output: &str) {
    for p in existing_tmp(settings, output) {
        let name = p.to_string_lossy().into_owned();
        let Some(ext) = files::image_extension(&name) else {
            continue;
        };
        if ext.starts_with(".img") {
            continue; // with its header
        }
        let stem = files::strip_image_extension(&name).to_owned();
        let Some(f) = ImageFiles::of(&name) else {
            continue;
        };
        if super::ssroi::copy_image(settings, &name, &stem).is_ok() {
            let target = settings
                .get("FSLOUTPUTTYPE")
                .and_then(|t| image::output_file(&stem, t))
                .map(|(n, _)| PathBuf::from(n));
            for q in std::iter::once(f.header).chain(f.data) {
                if target.as_ref() != Some(&q) {
                    let _ = std::fs::remove_file(q);
                }
            }
        }
    }
}

/// The files whose names start with `<output>_tmp` in the output's directory.
fn existing_tmp(settings: &Settings, output: &str) -> Vec<PathBuf> {
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

/// A file's size and modification time: a file the run did not create or change is not its own.
type Stamp = (u64, Option<std::time::SystemTime>);

fn stamp(p: &Path) -> Stamp {
    let m = std::fs::metadata(p).ok();
    (
        m.as_ref().map_or(0, std::fs::Metadata::len),
        m.and_then(|m| m.modified().ok()),
    )
}

/// The `<output>_tmp*` files before the run, with their stamps.
fn snapshot(settings: &Settings, output: &str) -> Vec<(PathBuf, Stamp)> {
    existing_tmp(settings, output)
        .into_iter()
        .map(|p| {
            let s = stamp(&p);
            (p, s)
        })
        .collect()
}

/// The cleanup (FSL-46): the run's own temporary files, every `<output>_tmp*` file its programs
/// created or changed (default), or every `<output>_tmp*` file (`reproduce`).
fn cleanup(
    settings: &Settings,
    output: &str,
    before: &[(PathBuf, Stamp)],
    mode: UpstreamDefects,
    notices: &mut Notices,
) {
    let now = existing_tmp(settings, output);
    let untouched = |p: &PathBuf| before.iter().any(|(q, s)| q == p && *s == stamp(p));
    let others: Vec<&PathBuf> = now.iter().filter(|p| untouched(p)).collect();
    if !others.is_empty() {
        let names: Vec<String> = others
            .iter()
            .map(|p| {
                p.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        let detail = match mode {
            UpstreamDefects::Fix => format!(
                "the cleanup removes only the run's own temporary files; the script also removes {}",
                names.join(", ")
            ),
            UpstreamDefects::Reproduce => format!(
                "the cleanup removes every {output}_tmp* file, as the script does: also {}",
                names.join(", ")
            ),
        };
        notices.add(&defects::FSL_46, detail);
    }
    let keep: Vec<PathBuf> = if mode.fix() {
        others.into_iter().cloned().collect()
    } else {
        Vec::new()
    };
    for p in now {
        if keep.contains(&p) {
            continue;
        }
        let _ = std::fs::remove_file(&p);
    }
}

/// `-R`: bet2 again from the previous brain's centre of gravity until it moves less than a
/// voxel (§5.2).
fn robust(
    run: &Run<'_>,
    input: &str,
    output: &str,
    opts: &[String],
    notices: &mut Notices,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), u8> {
    // FSL-63: the options through bash's echo, every -n removed
    let given: Vec<String> = opts.iter().filter(|o| *o != "-n").cloned().collect();
    let theirs = echo_minus_n(opts);
    let used = if run.mode.reproduce() {
        theirs.clone()
    } else {
        given.clone()
    };
    if theirs != given {
        let detail = match run.mode {
            UpstreamDefects::Fix => format!(
                "bet2's options are passed as given ({}); the script's echo makes them {}",
                given.join(" "),
                theirs.join(" ")
            ),
            UpstreamDefects::Reproduce => format!(
                "bet2's options pass through bash's echo as the script does: {}",
                theirs.join(" ")
            ),
        };
        notices.add(&defects::FSL_63, detail);
    }
    let mut args = vec![input.to_owned(), output.to_owned()];
    args.extend(used.iter().cloned());
    run.bet2(&args, out, err)?;
    let mut c = brain_centre(run, output, err).map_err(|_| run.failed(out))?;
    for _ in 0..10 {
        let mut a = args.clone();
        a.push("-c".into());
        a.extend(c.iter().map(|v| format!("{v:.6}")));
        run.bet2(&a, out, err)?;
        let next = brain_centre(run, output, err).map_err(|_| run.failed(out))?;
        let d2: f64 = (0..3).map(|k| (c[k] - next[k]) * (c[k] - next[k])).sum();
        c = next;
        if d2.trunc() < 1.0 {
            break;
        }
    }
    Ok(())
}

/// bash's `echo` of the options, every `-n` removed from its text (`-R`).
pub fn echo_minus_n(opts: &[String]) -> Vec<String> {
    echo_remove(opts, "-n")
}

/// bash's `echo` of the options, every occurrence of `text` removed from what it prints (`-R`'s
/// `-n`, `-B`'s `-s`): leading words made only of `-n`, `-e` and `-E` are echo's own options;
/// after a leading `-e` backslashes are escapes (FSL-63).
pub fn echo_remove(opts: &[String], text: &str) -> Vec<String> {
    let mut k = 0;
    let mut escapes = false;
    while k < opts.len() {
        let w = &opts[k];
        let own = w.len() >= 2
            && w.starts_with('-')
            && w[1..].chars().all(|c| matches!(c, 'n' | 'e' | 'E'));
        if !own {
            break;
        }
        for c in w[1..].chars() {
            match c {
                'e' => escapes = true,
                'E' => escapes = false,
                _ => {}
            }
        }
        k += 1;
    }
    let printed = opts[k..].join(" ");
    let printed = if escapes { unescape(&printed) } else { printed };
    printed
        .replace(text, "")
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

/// `echo -e`'s escapes (the common ones).
fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('\\') => out.push('\\'),
            Some('a') => out.push('\x07'),
            Some('b') => out.push('\x08'),
            Some('f') => out.push('\x0c'),
            Some('v') => out.push('\x0b'),
            Some('r') => out.push('\r'),
            Some('c') => break,
            Some(o) => {
                out.push('\\');
                out.push(o);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// The brain's centre of gravity in voxels of the file as stored, as the script computes it:
/// `fslstats <brain> -l 0.001 -C` (`specs/fslstats.md` §5.7: the voxels above 0.001, weighted by
/// their values less the minimum, the sums in FSL's blocks; its text, six decimals, read back),
/// the original's fslstats run in this process (its warning of an empty brain on standard error).
fn brain_centre(run: &Run<'_>, output: &str, err: &mut dyn Write) -> Result<[f64; 3], ()> {
    let args: Vec<String> = [output, "-l", "0.001", "-C"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let code = super::fslstats::main(&args, run.settings, &mut o, &mut e);
    let _ = err.write_all(&e);
    if code != 0 {
        return Err(());
    }
    let words: Vec<f64> = String::from_utf8_lossy(&o)
        .split_whitespace()
        .map(|w| w.parse::<f64>().map_err(|_| ()))
        .collect::<Result<_, _>>()?;
    match words[..] {
        [x, y, z] => Ok([x, y, z]),
        _ => Err(()),
    }
}

/// A failure of one of the script's steps on standard error: FSL's own message (the image
/// layer's `ERROR::` lines) as FSL prints it, anything else as larmorx's line.
fn report(err: &mut dyn Write, e: &str) {
    if e.starts_with("ERROR::") {
        let _ = err.write_all(e.as_bytes());
    } else {
        let _ = writeln!(err, "larmorx fsl bet: {e} (larmorx)");
    }
}

/// `header` with the qform's quaternion (`b`, `c`, `d` and the qfac) of `from`, as an FSL
/// program that read `from` holds it.
fn with_quaternion(
    header: &larmorx_io::nifti::NiftiHeader,
    from: &larmorx_io::nifti::NiftiHeader,
) -> larmorx_io::nifti::NiftiHeader {
    let mut h = header.clone();
    h.quatern_b = from.quatern_b;
    h.quatern_c = from.quatern_c;
    h.quatern_d = from.quatern_d;
    h.pixdim[0] = from.pixdim[0];
    h
}

/// Writes `vol` (radiological frame) as an image like `like`.
fn write_like(
    run: &Run<'_>,
    stem: &str,
    like: &image::Input,
    vols: &[Vol],
    values: Values,
    cal: Option<(f64, f64)>,
) -> Result<(), String> {
    let shape = vols.first().map_or(like.vol.shape, |v| v.shape);
    write_values(run, stem, like, shape, vols.len().max(1), values, cal)
}

/// [`write_like`] with the grid's shape and the number of volumes given.
fn write_values(
    run: &Run<'_>,
    stem: &str,
    like: &image::Input,
    shape: [usize; 3],
    volumes: usize,
    values: Values,
    cal: Option<(f64, f64)>,
) -> Result<(), String> {
    // FSL's programs (fslroi, fslmerge, fslmaths) print the image layer's messages (§3.1)
    let t = run
        .settings
        .get("FSLOUTPUTTYPE")
        .ok_or_else(|| bet2::FSLOUTPUTTYPE_UNSET.to_owned())?;
    let (name, version) = image::output_file(&run.settings.name(stem), t)
        .ok_or_else(|| format!("{}\n", image::unknown_output_type_message(t)))?;
    let o = Output {
        template: &like.header,
        flipped: like.flipped,
        shape,
        volumes,
        cal,
        extensions: false,
        defects: run.mode,
        n_threads: run.settings.threads(),
        descrip: image::DESCRIP,
        rewrite_quaternion: true,
    };
    image::write(&name, version, &o, values)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn read_input(run: &Run<'_>, name: &str) -> Result<(Vec<Vol>, image::Input), String> {
    let resolved = image::resolve_input(&run.settings.name(name))?;
    let f = ImageFiles::of(&resolved).ok_or_else(|| format!("{resolved}: not an image"))?;
    image::read_series_lean(&f, run.settings.threads()).map_err(|e| e.to_string())
}

/// The values of a temporary image fslroi or fslmerge writes from an image of `header`, in the
/// type they write (`specs/standard_space_roi.md` §3.2): the stored type read through its
/// scaling (uint16 as int32; a scaled image, uint32, the 64-bit integers and float64 as float32
/// here: bet2 reads them as single precision either way); int8 as uint8 with the same bytes in
/// FSL (FSL-217: `reproduce`), the default keeping int8 and its values.
fn roi_values(
    mode: UpstreamDefects,
    header: &larmorx_io::nifti::NiftiHeader,
    data: Vec<f32>,
) -> Values {
    let scaled = header.scl_slope.is_finite()
        && header.scl_slope != 0.0
        && (header.scl_slope != 1.0 || header.scl_inter != 0.0);
    if scaled {
        return Values::F32(data);
    }
    match header.datatype {
        2 => Values::U8(data.iter().map(|&x| x as u8).collect()),
        4 => Values::I16(data.iter().map(|&x| x as i16).collect()),
        8 | 512 => Values::I32(data.iter().map(|&x| x as i32).collect()),
        256 if mode.reproduce() => Values::U8(data.iter().map(|&x| (x as i8) as u8).collect()),
        256 => Values::I8(data.iter().map(|&x| x as i8).collect()),
        _ => Values::F32(data),
    }
}

/// FSL-217 named for `-F`'s and `-Z`'s temporary images of an int8 input: where a value is
/// negative (bet2 runs on other values in FSL), and by default also where `-d` keeps the int8
/// files (FSL's are uint8).
fn fsl217(
    mode: UpstreamDefects,
    header: &larmorx_io::nifti::NiftiHeader,
    data: &[f32],
    debug: bool,
    notices: &mut Notices,
) {
    if header.datatype != 256 {
        return;
    }
    let negative = data.iter().any(|&x| x < 0.0);
    match mode {
        UpstreamDefects::Fix if negative || debug => notices.add(
            &defects::FSL_217,
            "the int8 input is kept int8 with its values for bet2 (and in the temporary files); FSL's fslroi and fslmerge write it as uint8 with the same bytes, so FSL's bet2 runs on 128 to 255 for its negative values",
        ),
        UpstreamDefects::Reproduce if negative => notices.add(
            &defects::FSL_217,
            "the int8 input written as uint8 with the same bytes, as FSL's fslroi and fslmerge write it: bet2 runs on 128 to 255 for its negative values",
        ),
        _ => {}
    }
}

/// `-F`: bet2 on the first volume (`-m -n -f 0.3` and the user's options), its mask dilated by
/// one voxel, every volume masked (§5.2).
#[allow(clippy::too_many_arguments)]
fn fmri(
    run: &Run<'_>,
    input: &str,
    output: &str,
    opts: &[String],
    debug: bool,
    notices: &mut Notices,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), u8> {
    let fail = |e: String, out: &mut dyn Write, err: &mut dyn Write| {
        report(err, &e);
        run.failed(out)
    };
    let (vols, inp) = match read_input(run, input) {
        Ok(r) => r,
        Err(e) => return Err(fail(e, out, err)),
    };
    let tmp = format!("{output}_tmp");
    // the first volume as fslroi writes it: a double series's first volume keeps its double
    // values (observed, random testing 2026-10-31 case 1053, with -d)
    let nvox = vols[0].data.len();
    let first = if inp.header.datatype == 64 {
        image::resolve_input(&run.settings.name(input))
            .ok()
            .and_then(|r| ImageFiles::of(&r))
            .and_then(|f| image::read_f64_series(&f, run.settings.threads()))
            .filter(|d| d.len() >= nvox)
            .map(|mut d| {
                d.truncate(nvox);
                Values::F64(d)
            })
    } else {
        None
    };
    // the first volume in the input's type, as fslroi writes it (FSL-217 for int8)
    if first.is_none() {
        fsl217(run.mode, &inp.header, &vols[0].data, debug, notices);
    }
    let first = first.unwrap_or_else(|| roi_values(run.mode, &inp.header, vols[0].data.clone()));
    if let Err(e) = write_like(run, &tmp, &inp, &vols[..1], first, None) {
        return Err(fail(e, out, err));
    }
    let tmp_brain = format!("{tmp}_brain");
    let mut a = vec![
        tmp.clone(),
        tmp_brain.clone(),
        "-m".into(),
        "-n".into(),
        "-f".into(),
        "0.3".into(),
    ];
    a.extend(opts.iter().cloned());
    run.bet2(&a, out, err)?;
    let (mvols, minp) = match read_input(run, &format!("{tmp_brain}_mask")) {
        Ok(r) => r,
        Err(e) => return Err(fail(e, out, err)),
    };
    let dilated = dilate(&mvols[0]);
    if let Err(e) = write_like(
        run,
        &format!("{output}_mask"),
        &minp,
        &mvols[..1],
        Values::F32(dilated.clone()),
        Some((0.0, 0.0)),
    ) {
        return Err(fail(e, out, err));
    }
    // fslmaths -mas: the display range reset, float, or double for a double series with the
    // input's double values (observed, §5.2: only bet2's first volume is read as float)
    let (shape, count) = (vols[0].shape, vols.len());
    let doubles = if inp.header.datatype == 64 {
        image::resolve_input(&run.settings.name(input))
            .ok()
            .and_then(|r| ImageFiles::of(&r))
            .and_then(|f| image::read_f64_series(&f, run.settings.threads()))
            .filter(|d| d.len() == dilated.len() * count)
    } else {
        None
    };
    let values = if let Some(mut d) = doubles {
        drop(vols);
        for chunk in d.chunks_exact_mut(dilated.len()) {
            for (x, &m) in chunk.iter_mut().zip(&dilated) {
                if m == 0.0 {
                    *x = 0.0;
                }
            }
        }
        Values::F64(d)
    } else {
        // every volume masked, each freed as it is used: the series is held once
        let mut all = Vec::with_capacity(dilated.len() * count);
        for v in vols {
            all.extend(
                v.data
                    .iter()
                    .zip(&dilated)
                    .map(|(&x, &m)| if m != 0.0 { x } else { 0.0 }),
            );
        }
        if inp.header.datatype == 64 {
            Values::F64(all.into_iter().map(f64::from).collect())
        } else {
            Values::F32(all)
        }
    };
    drop(dilated);
    if let Err(e) = write_values(run, output, &inp, shape, count, values, Some((0.0, 0.0))) {
        return Err(fail(e, out, err));
    }
    Ok(())
}

/// `fslmaths -dilF`: the maximum over each voxel's 3 x 3 x 3 neighbourhood.
fn dilate(v: &Vol) -> Vec<f32> {
    let [nx, ny, nz] = v.shape;
    let mut out = vec![0f32; v.data.len()];
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let mut m = f32::NEG_INFINITY;
                for c in k.saturating_sub(1)..=(k + 1).min(nz - 1) {
                    for b in j.saturating_sub(1)..=(j + 1).min(ny - 1) {
                        for a in i.saturating_sub(1)..=(i + 1).min(nx - 1) {
                            m = m.max(v.data[(c * ny + b) * nx + a]);
                        }
                    }
                }
                out[(k * ny + j) * nx + i] = m;
            }
        }
    }
    out
}

/// `-Z`: the end slices repeated to 100 mm in z, bet2, the outputs cropped back (§5.2,
/// FSL-45).
#[allow(clippy::too_many_arguments)]
fn small_fov(
    run: &Run<'_>,
    input: &str,
    output: &str,
    opts: &[String],
    debug: bool,
    notices: &mut Notices,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), u8> {
    let fail = |e: String, out: &mut dyn Write, err: &mut dyn Write| {
        report(err, &e);
        run.failed(out)
    };
    let (vols, inp) = match read_input(run, input) {
        Ok(r) => r,
        Err(e) => return Err(fail(e, out, err)),
    };
    let v = &vols[0];
    let [nx, ny, nz] = v.shape;
    // fslval's pixdim3 (6 decimals), dc at scale 0
    let dz: f64 = format!("{:.6}", inp.header.pixdim[3].abs())
        .parse()
        .unwrap_or(1.0);
    let npad = (((100.0 / dz).trunc() - nz as f64) / 2.0).trunc() as i64 + 1;
    let mut plain = vec![input.to_owned(), output.to_owned()];
    plain.extend(opts.iter().cloned());
    if npad <= 0 {
        let _ = writeln!(
            out,
            "Image already has a greater Z FOV than 100.0 so not using padding"
        );
        return run.bet2(&plain, out, err);
    }
    let npad = npad as usize;
    let slice = nx * ny;
    let tmp = format!("{output}_tmp");
    // fslroi's slices and fslmerge's image in the input's type (FSL-217 for int8)
    fsl217(run.mode, &inp.header, &v.data, debug, notices);
    let typed = |data: Vec<f32>| roi_values(run.mode, &inp.header, data);
    let bottom = v.data[..slice].to_vec();
    let top = v.data[(nz - 1) * slice..].to_vec();
    let one = Vol::new([nx, ny, 1], v.voxel, bottom.clone());
    let w = |stem: &str, vol: &Vol, out: &mut dyn Write, err: &mut dyn Write| {
        write_like(
            run,
            stem,
            &inp,
            std::slice::from_ref(vol),
            typed(vol.data.clone()),
            None,
        )
        .map_err(|e| fail(e, out, err))
    };
    w(&format!("{tmp}_bottom"), &one, out, err)?;
    // the top slice as fslroi cuts it: its origin moved to that slice
    let mut top_inp = inp.clone();
    {
        let h = &mut top_inp.header;
        let shift = (nz - 1) as f64;
        for row in [&mut h.srow_x, &mut h.srow_y, &mut h.srow_z] {
            row[3] = f64::from((row[3] + row[2] * shift) as f32);
        }
        let r = crate::bet::quatern::quatern_to_mat(
            h.quatern_b,
            h.quatern_c,
            h.quatern_d,
            [h.pixdim[0], h.pixdim[1], h.pixdim[2], h.pixdim[3]],
        );
        // the qform moved only when it is set (observed: with code 0 its offset stays)
        if h.qform_code > 0 {
            h.qoffset_x = f64::from((h.qoffset_x + r[0][2] * shift) as f32);
            h.qoffset_y = f64::from((h.qoffset_y + r[1][2] * shift) as f32);
            h.qoffset_z = f64::from((h.qoffset_z + r[2][2] * shift) as f32);
        }
    }
    let top_vol = Vol::new([nx, ny, 1], v.voxel, top.clone());
    write_like(
        run,
        &format!("{tmp}_top"),
        &top_inp,
        std::slice::from_ref(&top_vol),
        typed(top_vol.data.clone()),
        None,
    )
    .map_err(|e| fail(e, out, err))?;
    let pz = nz + 2 * npad;
    let mut padded = Vec::with_capacity(slice * pz);
    for _ in 0..npad {
        padded.extend_from_slice(&bottom);
    }
    padded.extend_from_slice(&v.data);
    for _ in 0..npad {
        padded.extend_from_slice(&top);
    }
    let pvol = Vol::new([nx, ny, pz], v.voxel, padded);
    // fslmerge writes the padded image with the header of its first image, the bottom slice:
    // its quaternion is the one fslroi stored there, converted to the matrix and back once more
    // (each FSL program's read and write is one round trip, specs/bet.md §8.2, and the round
    // trip can move the last bits again)
    let bottom_header = read_input(run, &format!("{tmp}_bottom"))
        .map(|(_, b)| b.header)
        .map_err(|e| fail(e, out, err))?;
    let merged = image::Input {
        header: with_quaternion(&inp.header, &bottom_header),
        ..inp.clone()
    };
    write_like(
        run,
        &tmp,
        &merged,
        std::slice::from_ref(&pvol),
        typed(pvol.data.clone()),
        None,
    )
    .map_err(|e| fail(e, out, err))?;
    let mut a = vec![tmp.clone(), output.to_owned()];
    a.extend(opts.iter().cloned());
    run.bet2(&a, out, err)?;
    // the crop back
    let ours: Vec<String> = ["", "_mask", "_overlay", "_skull"]
        .iter()
        .map(|s| format!("{output}{s}"))
        .collect();
    // the PATH's folders by the platform's own separator (`;` on Windows, whose folders hold `:`)
    let path_has_imglob = std::env::split_paths(run.settings.get("PATH").unwrap_or(""))
        .any(|d| !d.as_os_str().is_empty() && d.join("imglob").is_file());
    let crop_list: Vec<String> = if run.mode.reproduce() {
        if path_has_imglob {
            let dir = run.settings.path(output);
            let base = dir
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_default();
            let parent = dir.parent().map(Path::to_path_buf).unwrap_or_default();
            let mut names: Vec<String> = std::fs::read_dir(if parent.as_os_str().is_empty() {
                Path::new(".")
            } else {
                &parent
            })
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                (n.starts_with(&base) && files::image_extension(&n).is_some())
                    .then(|| files::strip_image_extension(&n).to_owned())
            })
            .map(|n| {
                let p = Path::new(output).with_file_name(n);
                p.to_string_lossy().into_owned()
            })
            .collect();
            names.sort();
            names.dedup();
            notices.add(
                &defects::FSL_45,
                "-Z crops every image whose name starts with the output's, as the script's imglob finds them",
            );
            names
        } else {
            let _ = writeln!(err, "bet: line 329: imglob: command not found");
            notices.add(
                &defects::FSL_45,
                "-Z's outputs stay padded: the script's imglob is not on the PATH",
            );
            Vec::new()
        }
    } else {
        if !path_has_imglob {
            notices.add(
                &defects::FSL_45,
                "-Z's outputs are cropped back to the input's slices; the script calls imglob, which is not on the PATH, and leaves them padded",
            );
        }
        ours
    };
    for stem in crop_list {
        let Ok((pv, pinp)) = read_input(run, &stem) else {
            continue;
        };
        if pv[0].shape != [nx, ny, pz] {
            continue;
        }
        let data = pv[0].data[npad * slice..(npad + nz) * slice].to_vec();
        let cropped = Vol::new([nx, ny, nz], v.voxel, data);
        let is_int = pinp.header.datatype == 4;
        let values = if is_int {
            Values::I16(cropped.data.iter().map(|&x| x as i16).collect())
        } else {
            Values::F32(cropped.data.clone())
        };
        let cal = is_int.then_some((pinp.header.cal_min, pinp.header.cal_max));
        // fslroi's crop: the input's geometry, the quaternion the padded output holds (one
        // more round trip, §8.2)
        let like = image::Input {
            header: with_quaternion(&inp.header, &pinp.header),
            ..pinp
        };
        write_like(run, &stem, &like, &[cropped], values, cal).map_err(|e| fail(e, out, err))?;
    }
    Ok(())
}
