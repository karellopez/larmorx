// SPDX-License-Identifier: Apache-2.0
//! `larmorx fsl bet`: the `bet` script's command line and variations (`specs/bet.md` §5).
//!
//! The script runs bet2 (here [`super::bet2`], in this process) with the options it is given,
//! or one of its variations: `-R` (bet2 repeated from the brain's centre), `-F` (4D data), `-Z`
//! (a small field of view in z). `-S`, `-B`, `-A` and `-A2` run FSL programs larmorx does not
//! have yet and are refused as pending. Its messages are FSL's, its own path printed as `bet`.

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::{self, Notice, UpstreamDefects};
use larmorx_mri::hmc::files::{self, ImageFiles};

use super::Settings;
use super::bet2;
use crate::bet::Vol;
use crate::bet::image::{self, Output, Values};

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
    let mut notices: Vec<Notice> = Vec::new();
    // FSL-44: the script expands its names and options unquoted where it runs bet2 (a name
    // with a blank becomes several words), and an empty argument ends its options
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
        notices.push(Notice::new(&defects::FSL_44, mode, detail));
    }
    let words: Vec<String> = args.to_vec();
    if words.len() < 2 || words[1].is_empty() {
        let _ = out.write_all(USAGE.as_bytes());
        return 1;
    }
    let input = remove_ext(&words[0]).to_owned();
    let output = remove_ext(&words[1]).to_owned();
    let blank = mode.reproduce() && input.contains(char::is_whitespace);
    if blank || files::resolve_input(&settings.name(&input)).is_err() {
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
            "-d" => debug = true,
            _ => {
                let _ = out.write_all(USAGE.as_bytes());
                return 1;
            }
        }
        i += 1;
    }
    let _ = t2;
    if verbose {
        let _ = writeln!(out, "IN={input}");
        let _ = writeln!(out, "OUT={output}");
        let shown: String = opts.iter().map(|o| format!(" {o}")).collect();
        let _ = writeln!(out, "bet2opts={shown}");
        let _ = writeln!(out, "verbose=1");
        let _ = writeln!(out, "debug={}", u8::from(debug));
        let _ = writeln!(out, "variation={}", variation as u8);
    }
    let pending = match variation {
        Variation::Eyes => Some(("-S", "standard_space_roi, FLIRT and fslmaths' morphology")),
        Variation::Bias => Some(("-B", "standard_space_roi and FAST")),
        Variation::Surfaces => Some(("-A", "FLIRT (then betsurf)")),
        Variation::SurfacesT2 => Some(("-A2", "FLIRT (then betsurf)")),
        _ => None,
    };
    if let Some((flag, need)) = pending {
        let _ = writeln!(
            err,
            "larmorx fsl bet: {flag} is pending: it runs FSL's {need}, which larmorx does not have yet (larmorx)"
        );
        return 1;
    }
    let run = Run {
        settings,
        mode,
        args,
    };
    let before = existing_tmp(settings, &output);
    let result = match variation {
        Variation::Robust => robust(&run, &input, &output, &opts, &mut notices, out, err),
        Variation::Fmri => fmri(&run, &input, &output, &opts, out, err),
        Variation::SmallFov => small_fov(&run, &input, &output, &opts, &mut notices, out, err),
        _ => {
            let mut a = vec![input.clone(), output.clone()];
            a.extend(opts.iter().cloned());
            run.bet2(&a, out, err)
        }
    };
    if !debug {
        cleanup(settings, &output, &before, mode, &mut notices);
    }
    for n in &notices {
        let _ = writeln!(err, "larmorx: {}", n.line());
    }
    match result {
        Ok(()) => 0,
        Err(code) => code,
    }
}

/// The files whose names start with `<output>_tmp` in the output's directory.
fn existing_tmp(settings: &Settings, output: &str) -> Vec<std::path::PathBuf> {
    let full = settings.path(&format!("{output}_tmp"));
    let dir = full
        .parent()
        .map(Path::to_path_buf)
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
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

/// The cleanup (FSL-46): the run's own temporary files (default), or every `<output>_tmp*`
/// file (`reproduce`).
fn cleanup(
    settings: &Settings,
    output: &str,
    before: &[std::path::PathBuf],
    mode: UpstreamDefects,
    notices: &mut Vec<Notice>,
) {
    let now = existing_tmp(settings, output);
    let others: Vec<_> = before.iter().filter(|p| now.contains(p)).collect();
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
        notices.push(Notice::new(&defects::FSL_46, mode, detail));
    }
    for p in now {
        if mode.fix() && before.contains(&p) {
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
    notices: &mut Vec<Notice>,
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
        notices.push(Notice::new(&defects::FSL_63, run.mode, detail));
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

/// bash's `echo` of the options, every `-n` removed from its text: leading words made only of
/// `-n`, `-e` and `-E` are echo's own options; after a leading `-e` backslashes are escapes.
pub fn echo_minus_n(opts: &[String]) -> Vec<String> {
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
    let text = opts[k..].join(" ");
    let text = if escapes { unescape(&text) } else { text };
    text.replace("-n", "")
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

/// The output's image file for the run's `FSLOUTPUTTYPE`.
fn image_name(run: &Run<'_>, stem: &str) -> Option<String> {
    let t = run.settings.get("FSLOUTPUTTYPE")?;
    image::output_file(&run.settings.name(stem), t).map(|(n, _)| n)
}

/// The brain's centre of gravity in voxels of the file as stored (`fslstats -C`): the mean
/// voxel coordinates of the voxels above 0.001, weighted by their values, to 6 decimals.
fn brain_centre(run: &Run<'_>, output: &str, err: &mut dyn Write) -> Result<[f64; 3], ()> {
    let name = image_name(run, output).ok_or(())?;
    let f = ImageFiles::of(&name).ok_or(())?;
    let (_, inp) = image::read(&f, run.settings.threads()).map_err(|_| ())?;
    let [nx, ny, nz] = inp.vol.shape;
    let mut s = [0f64; 4];
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let v = f64::from(inp.vol.data[(k * ny + j) * nx + i]);
                if v > 0.001 {
                    let x = if inp.flipped { nx - 1 - i } else { i };
                    s[0] += v * x as f64;
                    s[1] += v * j as f64;
                    s[2] += v * k as f64;
                    s[3] += v;
                }
            }
        }
    }
    if s[3] < 1e-5 {
        let _ = writeln!(err, "WARNING::in calculating COG, total = 0.0");
        s[3] = 1.0;
    }
    Ok([0, 1, 2].map(|k| {
        let v = s[k] / s[3];
        format!("{v:.6}").parse().unwrap_or(v)
    }))
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
    let t = run
        .settings
        .get("FSLOUTPUTTYPE")
        .ok_or_else(|| "FSLOUTPUTTYPE is not set".to_owned())?;
    let (name, version) = image::output_file(&run.settings.name(stem), t)
        .ok_or_else(|| format!("FSLOUTPUTTYPE={t} is not a type"))?;
    let o = Output {
        template: &like.header,
        flipped: like.flipped,
        shape,
        volumes,
        cal,
        extensions: false,
        defects: run.mode,
        n_threads: run.settings.threads(),
    };
    image::write(&name, version, &o, values)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn read_input(run: &Run<'_>, name: &str) -> Result<(Vec<Vol>, image::Input), String> {
    let resolved = files::resolve_input(&run.settings.name(name))?;
    let f = ImageFiles::of(&resolved).ok_or_else(|| format!("{resolved}: not an image"))?;
    image::read(&f, run.settings.threads()).map_err(|e| e.to_string())
}

/// `-F`: bet2 on the first volume (`-m -n -f 0.3` and the user's options), its mask dilated by
/// one voxel, every volume masked (§5.2).
fn fmri(
    run: &Run<'_>,
    input: &str,
    output: &str,
    opts: &[String],
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), u8> {
    let fail = |e: String, out: &mut dyn Write, err: &mut dyn Write| {
        let _ = writeln!(err, "larmorx fsl bet: {e} (larmorx)");
        run.failed(out)
    };
    let (vols, inp) = match read_input(run, input) {
        Ok(r) => r,
        Err(e) => return Err(fail(e, out, err)),
    };
    let tmp = format!("{output}_tmp");
    if let Err(e) = write_like(
        run,
        &tmp,
        &inp,
        &vols[..1],
        Values::F32(vols[0].data.clone()),
        None,
    ) {
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
    // every volume masked, each freed as it is used: the series is held once
    let (shape, count) = (vols[0].shape, vols.len());
    let mut all = Vec::with_capacity(dilated.len() * count);
    for v in vols {
        all.extend(
            v.data
                .iter()
                .zip(&dilated)
                .map(|(&x, &m)| if m != 0.0 { x } else { 0.0 }),
        );
    }
    drop(dilated);
    // fslmaths -mas: the display range reset (observed)
    if let Err(e) = write_values(
        run,
        output,
        &inp,
        shape,
        count,
        Values::F32(all),
        Some((0.0, 0.0)),
    ) {
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
fn small_fov(
    run: &Run<'_>,
    input: &str,
    output: &str,
    opts: &[String],
    notices: &mut Vec<Notice>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), u8> {
    let fail = |e: String, out: &mut dyn Write, err: &mut dyn Write| {
        let _ = writeln!(err, "larmorx fsl bet: {e} (larmorx)");
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
    let bottom = v.data[..slice].to_vec();
    let top = v.data[(nz - 1) * slice..].to_vec();
    let one = Vol::new([nx, ny, 1], v.voxel, bottom.clone());
    let w = |stem: &str, vol: &Vol, out: &mut dyn Write, err: &mut dyn Write| {
        write_like(
            run,
            stem,
            &inp,
            std::slice::from_ref(vol),
            Values::F32(vol.data.clone()),
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
        h.qoffset_x = f64::from((h.qoffset_x + r[0][2] * shift) as f32);
        h.qoffset_y = f64::from((h.qoffset_y + r[1][2] * shift) as f32);
        h.qoffset_z = f64::from((h.qoffset_z + r[2][2] * shift) as f32);
    }
    let top_vol = Vol::new([nx, ny, 1], v.voxel, top.clone());
    write_like(
        run,
        &format!("{tmp}_top"),
        &top_inp,
        std::slice::from_ref(&top_vol),
        Values::F32(top_vol.data.clone()),
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
    w(&tmp, &pvol, out, err)?;
    let mut a = vec![tmp.clone(), output.to_owned()];
    a.extend(opts.iter().cloned());
    run.bet2(&a, out, err)?;
    // the crop back
    let ours: Vec<String> = ["", "_mask", "_overlay", "_skull"]
        .iter()
        .map(|s| format!("{output}{s}"))
        .collect();
    let path_has_imglob = run
        .settings
        .get("PATH")
        .unwrap_or("")
        .split(':')
        .any(|d| !d.is_empty() && Path::new(d).join("imglob").is_file());
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
            notices.push(Notice::new(
                &defects::FSL_45,
                run.mode,
                "-Z crops every image whose name starts with the output's, as the script's imglob finds them",
            ));
            names
        } else {
            let _ = writeln!(err, "bet: line 329: imglob: command not found");
            notices.push(Notice::new(
                &defects::FSL_45,
                run.mode,
                "-Z's outputs stay padded: the script's imglob is not on the PATH",
            ));
            Vec::new()
        }
    } else {
        if !path_has_imglob {
            notices.push(Notice::new(
                &defects::FSL_45,
                run.mode,
                "-Z's outputs are cropped back to the input's slices; the script calls imglob, which is not on the PATH, and leaves them padded",
            ));
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
        let like = image::Input {
            header: inp.header.clone(),
            ..pinp
        };
        write_like(run, &stem, &like, &[cropped], values, cal).map_err(|e| fail(e, out, err))?;
    }
    Ok(())
}
