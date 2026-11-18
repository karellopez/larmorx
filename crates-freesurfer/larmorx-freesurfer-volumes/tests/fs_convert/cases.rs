// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs-convert probe's cases (`validation/probes/fs-convert/`): a seeded generator of case lines
//! for `mri_convert`'s stats tables, CSV reader and command line, and the port's answer to each,
//! printed as `fsconvert` prints FreeSurfer's (`<result> | <text>`, the text C-escaped; `EXIT
//! <status> | <text>` when the case ended the program), each followed by the files it wrote.
#![allow(dead_code)]

use std::cell::RefCell;
use std::io::Write;
use std::path::Path;
use std::rc::Rc;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::Matrix;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_io::mriio::mri_read_volume;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_INT, MRI_SHORT, MRI_UCHAR, Mri, mri_alloc_sequence};
use larmorx_freesurfer_volumes::mri_convert::csv::read_csv;
use larmorx_freesurfer_volumes::mri_convert::options::{Options, Run, parse};
use larmorx_freesurfer_volumes::mri_convert::stat_table::{
    Io, StatTable, init_stat_table_from_mri, load_stat_table, write_stat_table,
};

/// The cases' random numbers.
pub struct Gen(u64);

impl Gen {
    pub fn new(seed: u64) -> Gen {
        Gen(seed)
    }
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn int(&mut self, lo: i64, hi: i64) -> i64 {
        lo + self.below((hi - lo + 1) as u64) as i64
    }
    fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
        xs[self.below(xs.len() as u64) as usize]
    }
    fn chance(&mut self, k: u64) -> bool {
        self.below(k) == 0
    }
}

fn hex(b: &[u8]) -> String {
    if b.is_empty() {
        return "@".to_owned();
    }
    b.iter().map(|c| format!("{c:02x}")).collect()
}

fn unhex(h: &str) -> Vec<u8> {
    if h == "@" {
        return Vec::new();
    }
    (0..h.len() / 2)
        .map(|i| u8::from_str_radix(&h[2 * i..2 * i + 2], 16).expect("hex"))
        .collect()
}

// ------------------------------------------------------------------ the generator

const NUMBERS: &[&str] = &[
    "1", "0", "-2.5", "3.25", "1e3", "nan", "-nan", "inf", "-inf", "1e", "0x1p3", "1e39", "1e-50",
    "1e-40", "2.", ".5", "+7", "abc", "1.5x", "0x", "infinity", "nan(12)", "-0", "1e308", "1e-320",
];

fn word(g: &mut Gen) -> String {
    let w = ["Left-Hippo", "x", "Volume_mm3", "a\"b", "c,d", "lh", "rh.thickness", "BrainSeg"];
    let mut s = g.pick(&w).to_owned();
    if g.chance(5) {
        s += &g.int(0, 99).to_string();
    }
    s
}

fn sep(g: &mut Gen) -> &'static str {
    g.pick(&[" ", " ", "  ", "\t", " \t "])
}

fn eol(g: &mut Gen) -> &'static str {
    g.pick(&["\n", "\n", "\n", "\r\n", " \n"])
}

/// A table in FreeSurfer's format (or close to it).
fn fs_table(g: &mut Gen) -> Vec<u8> {
    if g.chance(15) {
        return Vec::new();
    }
    let ncols = g.int(0, 4) as usize;
    let nrows = g.int(0, 4) as usize;
    let mut s = String::new();
    s += &word(g);
    for _ in 0..ncols {
        s += sep(g);
        s += &word(g);
    }
    s += eol(g);
    for r in 0..nrows {
        if g.chance(8) {
            s += "\n";
        }
        s += &format!("row{r}");
        let n = if g.chance(8) {
            ncols.saturating_sub(1)
        } else if g.chance(10) {
            ncols + 1
        } else {
            ncols
        };
        for _ in 0..n {
            s += sep(g);
            s += if g.chance(4) {
                g.pick(NUMBERS)
            } else {
                g.pick(&["1", "2.5", "-3", "100.125", "0.001"])
            };
        }
        if r + 1 < nrows || !g.chance(4) {
            s += eol(g);
        }
    }
    if g.chance(5) {
        s += "\n";
    }
    if g.chance(40) {
        // a line or a word longer than the readers' buffer
        let long = "7".repeat(100_000 + g.int(0, 3) as usize);
        s = format!("m c{}\nr {long}\n", if g.chance(2) { "" } else { " d" });
    }
    s.into_bytes()
}

/// A CSV text.
fn csv_text(g: &mut Gen) -> Vec<u8> {
    if g.chance(12) {
        return Vec::new();
    }
    let ncols = g.int(0, 3) as usize;
    let nrows = g.int(0, 4) as usize;
    let field = |g: &mut Gen, num: bool| -> String {
        if num && !g.chance(6) {
            return g
                .pick(&["1", "2.5", " -3", "4e2", "0.5 ", "\"7\"", "8x"])
                .to_owned();
        }
        if num {
            return g.pick(NUMBERS).to_owned();
        }
        g.pick(&["name", "\"quoted,field\"", "a\"\"b", "\"x\"y", "", " sp ", "z\r"])
            .to_owned()
    };
    let mut s = field(g, false);
    for _ in 0..ncols {
        s += ",";
        s += &field(g, false);
    }
    s += "\n";
    for _ in 0..nrows {
        if g.chance(8) {
            s += "\n";
        }
        s += &field(g, false);
        let n = if g.chance(8) {
            ncols.saturating_sub(1)
        } else {
            ncols + usize::from(g.chance(10))
        };
        for _ in 0..n {
            s += ",";
            s += &field(g, true);
        }
        s += g.pick(&["\n", "\n", "\r\n"]);
    }
    if g.chance(4) {
        s.pop();
    }
    s.into_bytes()
}

/// A TSV text (`TSV::read`'s format), for `.tsv` or `.tac`.
fn tsv_text(g: &mut Gen, tac: bool) -> Vec<u8> {
    if g.chance(15) {
        return Vec::new();
    }
    let ncols = g.int(1, 4) as usize;
    let mut heads: Vec<String> = (0..ncols).map(|c| format!("h{c}")).collect();
    if tac && ncols >= 2 && g.chance(2) {
        heads[1] = "end[kBq/cc]".to_owned();
    }
    let mut s = heads.join("\t") + "\n";
    let nrows = g.int(0, 3);
    for _ in 0..nrows {
        // never an entry that is not a number: FreeSurfer's TSV reader then reads rows for ever
        // (FS-172, fs.io's probe)
        let vals: Vec<&str> = (0..ncols)
            .map(|_| g.pick(&["1", "2.5", "-0.25", "3e1", "nan", "1e400"]))
            .collect();
        s += &vals.join("\t");
        s += "\n";
    }
    if g.chance(8) {
        s += "1\t";
    }
    s.into_bytes()
}

fn fx(v: f32) -> String {
    format!("{:08x}", v.to_bits())
}

/// A small volume's tokens: `w h d type nframes xsize ysize zsize` and its voxels.
fn vol_tokens(g: &mut Gen, w: i32, nframes: i32) -> String {
    let ty = g.pick(&[MRI_FLOAT, MRI_FLOAT, MRI_UCHAR, MRI_INT, MRI_SHORT]);
    let mut t = vec![
        w.to_string(),
        "1".into(),
        "1".into(),
        ty.to_string(),
        nframes.to_string(),
    ];
    for _ in 0..3 {
        t.push(fx(1.0));
    }
    for _ in 0..(w * nframes) {
        let v = g.int(-3, 200);
        t.push(match ty {
            MRI_FLOAT => fx(v as f32 + 0.125 * g.int(0, 7) as f32),
            MRI_UCHAR => v.clamp(0, 255).to_string(),
            _ => v.to_string(),
        });
    }
    t.join(" ")
}

/// A template for `--out_stats_table` of `w` columns and about `nframes` rows.
fn template(g: &mut Gen, w: i32, nframes: i32) -> (String, String) {
    match g.below(8) {
        0 => ("-".into(), "-".into()),
        1 => ("@".into(), "-".into()),
        2 => ("in_missing.dat".into(), "-".into()),
        _ => {
            let ncols = if g.chance(5) { w + 1 } else { w } as usize;
            let nrows = (i64::from(nframes) + g.int(-1, 1)).max(0) as usize;
            let mut s = "Measure".to_owned();
            for c in 0..ncols {
                s += &format!(" c{c}");
            }
            s += "\n";
            for r in 0..nrows {
                s += &format!("r{r} 1 2 3\n");
                if g.chance(6) {
                    s += "\n";
                }
            }
            if g.chance(10) {
                s.clear();
            }
            ("in_tmpl.dat".into(), hex(s.as_bytes()))
        }
    }
}

/// An argument of an option taking integers.
fn arg_int(g: &mut Gen) -> String {
    if g.chance(6) {
        return g
            .pick(&["", " 5", "+4", "12x", "2147483648", "99999999999999999999", "0x10", "1e3", "-"])
            .to_owned();
    }
    g.int(-3, 30).to_string()
}

/// An argument of an option taking numbers.
fn arg_float(g: &mut Gen) -> String {
    if g.chance(5) {
        return g.pick(NUMBERS).to_owned();
    }
    g.pick(&["1", "0.5", "-2", "3.75", "0", "1.25", "256", "0.001", "1e-3", ""])
        .to_owned()
}

/// The options and their arguments.
#[derive(Clone, Copy)]
enum Kind {
    Flag,
    Ints(usize),
    Floats(usize),
    Atof,
    Str,
    /// A choice: the first `k` are accepted.
    Pick(usize, &'static [&'static str]),
    Matrix,
    Matrix2,
    Exists,
    Write,
    Read,
    Frames,
    Scan(usize),
    Raw,
    Rare,
}

const FLAGS: &[&str] = &[
    "--debug", "--left-right-reverse", "--left-right-reverse-pix", "--left-right-swap-label",
    "--flip-cols", "--slice-reverse", "--ascii", "--ascii+crsf", "--ascii-fcol", "--in_stats_table",
    "--out_stats_table", "-c", "--conform", "--conform-dc", "--cw256", "--delete-cmds", "--sphinx",
    "--no-rescale-dicom", "--bvec-scanner", "--bvec-voxel", "--no-analyze-rescale", "-nc",
    "--nochange", "-so", "--store_orig_ras2vox", "-cm", "--conform_min", "-po", "--parse_only",
    "-ii", "--in_info", "-oi", "--out_info", "-ti", "--template_info", "-is", "--in_stats", "-os",
    "--out_stats", "-ro", "--read_only", "-nw", "--no_write", "-im", "--in_matrix", "-om",
    "--out_matrix", "--force_ras_good", "--split", "--antialias", "-ds2", "--downsample2",
    "--bfile-little-endian", "-dicomread3", "-dcm2niix", "--dcm2niix", "-no-dcm2niix",
    "--no-dcm2niix", "-createBIDS", "--createBIDS", "-dcm2niix-createBIDS",
    "--dcm2niix-createBIDS", "-dcm2niix-no-ForceStackSameSeries",
    "--dcm2niix-no-ForceStackSameSeries", "-dcm2niix-singlefile", "--dcm2niix-singlefile",
    "-siemens-ascii-dump", "-siemens-ascii-alt-dump", "-dicomread2", "-dicomread0",
    "-siemensBVecsCross", "-no-siemensBVecsCross", "--mid-frame", "-roi", "--roi", "-fp",
    "--fill_parcellation", "-zo", "--zero_outlines", "-nt", "--no_translate", "-cg", "--crop_gdf",
    "--mosaic-fix-noascii", "--no-strip-pound", "-zgez", "--zero_ge_z_offset", "-nozgez",
    "--no_zero_ge_z_offset", "--diag-debug", "--mra", "--auto-slice-res", "--no-dwi", "--no-ctab",
    "--remove-ctab",
];

const TYPES: &[&str] = &["mgz", "nii", "MGZ", "mgh", "spm", "analyze", "cor", "gdf", "otl", "bogus"];

fn options() -> Vec<(&'static [&'static str], Kind)> {
    vec![
        (&["-oval", "--outside_val", "--upsample", "--nthreads", "--threads", "--reduce"], Kind::Ints(1)),
        (&["-oni", "-oic", "--out_i_count", "-onj", "-ojc", "--out_j_count", "-onk", "-okc", "--out_k_count"], Kind::Ints(1)),
        (&["-ini", "-iic", "--in_i_count", "-inj", "-ijc", "--in_j_count", "-ink", "-ikc", "--in_k_count"], Kind::Ints(1)),
        (&["--erode-seg", "--dil-seg", "--cutends", "-ns", "--no_scale", "-nth", "--nth_frame"], Kind::Ints(1)),
        (&["-sp", "--smooth_parcellation"], Kind::Pick(3, &["14", "26", "20", "13", "27"])),
        (&["--nslices-override", "--ncols-override", "--nrows-override", "--nskip", "--ndrop", "--diag"], Kind::Ints(1)),
        (&["-r", "--reorder", "--shift", "--crop", "--cropsize"], Kind::Ints(3)),
        (&["-r4", "--reorder4"], Kind::Ints(4)),
        (&["--slice-crop"], Kind::Ints(2)),
        (&["--fsubsample"], Kind::Ints(3)),
        (&["-ut", "--upper_thresh", "--invert_contrast", "-cs", "--conform_size", "--fwhm", "-tr", "-TI", "-te", "-flip_angle", "--slice-bias"], Kind::Floats(1)),
        (&["-iis", "--in_i_size", "-ijs", "--in_j_size", "-iks", "--in_k_size", "-ois", "--out_i_size", "-ojs", "--out_j_size", "-oks", "--out_k_size"], Kind::Floats(1)),
        (&["-iid", "--in_i_direction", "-ijd", "--in_j_direction", "-ikd", "--in_k_direction", "-oid", "--out_i_direction", "-ojd", "--out_j_direction", "-okd", "--out_k_direction"], Kind::Floats(3)),
        (&["-ic", "--in_center", "-dic", "--delta_in_center", "-oc", "--out_center", "-vs", "--voxsize", "-voxsize", "-ds", "--downsample", "-downsample", "-dsold", "--downsampleold", "-downsampleold"], Kind::Floats(3)),
        (&["--rescale", "-sc", "--scale", "-osc", "--out-scale"], Kind::Atof),
        (&["--rescale-voxel"], Kind::Scan(3)),
        (&["--nspmzeropad", "--out_nspmzeropad", "--in_nspmzeropad"], Kind::Scan(1)),
        (&["-i", "--input_volume", "-o", "--output_volume", "--new-transform-fname", "-at", "--apply_transform", "-T", "--like", "--devolvexfm", "--ctab", "-sn", "--subject_name", "-gis", "--gdf_image_stem", "-rl", "--reslice_like", "--dil-seg-mask", "-il", "--in_like", "--first-dicom", "-cf", "--color_file"], Kind::Str),
        (&["-dcm2niix-outdir", "--dcm2niix-outdir", "-dcm2niix-info-dump", "--dcm2niix-info-dump", "-dcm2niix-dicom-flist", "--dcm2niix-dicom-flist", "-dcm2niix-opts", "--dcm2niix-opts"], Kind::Raw),
        (&["-odt", "--out_data_type"], Kind::Pick(5, &["uchar", "SHORT", "int", "Float", "rgb", "double", ""])),
        (&["-rt", "--resample_type"], Kind::Pick(5, &["interpolate", "NEAREST", "vote", "weighted", "cubic", "sinc", "bad"])),
        (&["-it", "--in_type", "-ot", "--out_type", "-tt", "--template_type"], Kind::Pick(4, TYPES)),
        (&["-io", "--in_orientation", "-oo", "--out_orientation"], Kind::Pick(5, &["LIA", "RAS", "lia", "PSL", "LIAS", "LLA", "xyz", "LI", "R1A"])),
        (&["--left-right-mirror", "--left-right-keep"], Kind::Pick(2, &["lh", "rh", "LH", "both"])),
        (&["--left-right-swap-label-table"], Kind::Matrix2),
        (&["--autoalign"], Kind::Matrix),
        (&["-ait", "--apply_inverse_transform"], Kind::Exists),
        (&["--status", "--statusfile"], Kind::Write),
        (&["--sdcmlist"], Kind::Read),
        (&["-f", "--frame"], Kind::Frames),
        (&["-u", "--usage", "--help", "-h", "-version2", "--version", "--bogus", "-x"], Kind::Rare),
        (FLAGS, Kind::Flag),
    ]
}

const OUT_NAMES: &[&str] = &[
    "out.mgz", "out.mgz", "out.nii.gz", "out.nii", "out.mnc", "out.img", "out.xyz", "outname",
    "out.mgh", "out.gdf", "COR-",
];

const VALID_OUT: &[&str] = &["out.mgz", "out.nii.gz", "out.nii", "out.mnc", "out.img", "out.mgh"];

/// An argument of an option taking integers (`valid`: one it accepts).
fn int_arg(g: &mut Gen, valid: bool) -> String {
    if valid {
        g.int(0, 30).to_string()
    } else {
        arg_int(g)
    }
}

/// An argument of an option taking numbers (`valid`: one it accepts, not 0).
fn float_arg(g: &mut Gen, valid: bool) -> String {
    if valid {
        g.pick(&["1", "0.5", "-2", "3.75", "1.25", "256", "1e-3", "2.", ".5", "nan", "inf"])
            .to_owned()
    } else if g.chance(6) {
        "0".into()
    } else {
        arg_float(g)
    }
}

/// One `opt` case: its files and arguments. Three of four cases give only arguments the options
/// accept (their combinations may still conflict); the others anything.
fn opt_case(g: &mut Gen, specs: &[(&'static str, Kind)]) -> String {
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut args: Vec<String> = Vec::new();
    let valid = !g.chance(4);
    let nopt = if valid { g.int(1, 3) } else { g.int(0, 4) };
    for _ in 0..nopt {
        let (name, kind) = specs[g.below(specs.len() as u64) as usize];
        if matches!(kind, Kind::Rare) && (valid || !g.chance(3)) {
            continue;
        }
        args.push(name.to_owned());
        let short = !valid && g.chance(8);
        match kind {
            Kind::Flag | Kind::Rare => {}
            Kind::Ints(n) => {
                let n = if short { g.below(n as u64) as usize } else { n };
                for _ in 0..n {
                    args.push(int_arg(g, valid));
                }
                if name == "--slice-crop" && n == 2 && (valid || g.chance(2)) {
                    let k = args.len();
                    args[k - 2] = "2".into();
                    args[k - 1] = if valid { "5" } else { "1" }.into();
                }
                if name == "--fsubsample" && n == 3 && !valid && g.chance(3) {
                    let k = args.len();
                    args[k - 2] = "0".into();
                } else if name == "--fsubsample" && n == 3 {
                    let k = args.len();
                    args[k - 2] = "2".into();
                }
            }
            Kind::Floats(n) => {
                let n = if short { g.below(n as u64) as usize } else { n };
                for _ in 0..n {
                    args.push(float_arg(g, valid));
                }
            }
            Kind::Atof => {
                if !short {
                    args.push(float_arg(g, valid));
                }
            }
            Kind::Scan(n) => {
                let n = if short { g.below(n as u64) as usize } else { n };
                for _ in 0..n {
                    args.push(if !valid && g.chance(5) {
                        "x".into()
                    } else {
                        int_arg(g, valid)
                    });
                }
            }
            Kind::Str => {
                if !short {
                    args.push(if !valid && g.chance(20) {
                        "n".repeat(g.pick(&[1023usize, 1024, 2000]))
                    } else if valid {
                        g.pick(&["name", "sub01", "a b", "in_x.lta", "lh.mgz"]).to_owned()
                    } else {
                        g.pick(&["name", "", "sub01"]).to_owned()
                    });
                }
            }
            Kind::Raw => {
                if !short {
                    args.push(g.pick(&["dir", "opts", ""]).to_owned());
                }
            }
            Kind::Pick(k, xs) => {
                if !short {
                    let n = if valid { k } else { xs.len() };
                    args.push(xs[g.below(n as u64) as usize].to_owned());
                }
            }
            Kind::Matrix | Kind::Matrix2 => {
                let k = if matches!(kind, Kind::Matrix2) { 2 } else { 1 };
                let k = if short { g.below(k) } else { k };
                for j in 0..k {
                    let f = format!("in_m{j}.txt");
                    match if valid { 2 } else { g.below(4) } {
                        0 => {}
                        1 => files.push((f.clone(), b"1 2\n3 x\n".to_vec())),
                        _ => files.push((
                            f.clone(),
                            b"1 0 0 4\n0 1 0 -2\n0 0 1 3.5\n0 0 0 1\n".to_vec(),
                        )),
                    }
                    args.push(f);
                }
            }
            Kind::Exists => {
                if !short {
                    if valid || g.chance(2) {
                        files.push(("in_t.lta".into(), b"x\n".to_vec()));
                    }
                    args.push("in_t.lta".into());
                }
            }
            Kind::Write => {
                if !short {
                    let f = if valid {
                        "status.txt"
                    } else {
                        g.pick(&["status.txt", "nodir/status.txt"])
                    };
                    args.push(f.to_owned());
                }
            }
            Kind::Read => {
                if !short {
                    if valid || g.chance(2) {
                        files.push(("in_list.txt".into(), b"a\n".to_vec()));
                    }
                    args.push("in_list.txt".into());
                }
            }
            Kind::Frames => {
                let n = if short { 0 } else { g.int(1, 4) };
                for _ in 0..n {
                    args.push(if !valid && g.chance(8) {
                        "x".into()
                    } else {
                        g.int(0, 9).to_string()
                    });
                }
            }
        }
    }
    // the volumes, among the options
    let names: Vec<String> = if valid {
        // the names `-i` and `-o` do not give (the first positional name fills the input when
        // it is still empty, else the output)
        let given = |o: &[&str]| args.iter().any(|a| o.contains(&a.as_str()));
        let (i, o) = (given(&["-i", "--input_volume"]), given(&["-o", "--output_volume"]));
        let input = g.pick(&["in.mgz", "in.mgz", "in.nii.gz", "in.mgz@mgh", "in.mgz#3"]);
        let out = g.pick(VALID_OUT);
        match (i, o) {
            (false, false) => vec![input.into(), out.into()],
            (true, false) => vec![out.into()],
            (false, true) => vec![input.into()],
            (true, true) => vec![],
        }
    } else {
        match g.below(6) {
            0 => vec![],
            1 => vec!["in.mgz".into()],
            2 => vec!["in.mgz".into(), g.pick(OUT_NAMES).into(), "extra.mgz".into()],
            _ => vec!["in.mgz".into(), g.pick(OUT_NAMES).into()],
        }
    };
    for n in names {
        let at = g.below(args.len() as u64 + 1) as usize;
        // never between an option and its arguments: before an option or at the end
        let at = (at..args.len())
            .find(|&k| args[k].starts_with('-') && args[k].len() > 1)
            .unwrap_or(args.len());
        args.insert(at, n);
    }
    let mut t = vec!["opt".to_owned(), files.len().to_string()];
    for (f, c) in &files {
        t.push(f.clone());
        t.push(hex(c));
    }
    t.push(args.len().to_string());
    t.extend(args.iter().map(|a| hex(a.as_bytes())));
    t.join(" ")
}

/// An `opt` case of the given files and arguments.
fn opt_line(files: &[(&str, &[u8])], args: &[&str]) -> String {
    let mut t = vec!["opt".to_owned(), files.len().to_string()];
    for (f, c) in files {
        t.push((*f).to_owned());
        t.push(hex(c));
    }
    t.push(args.len().to_string());
    t.extend(args.iter().map(|a| hex(a.as_bytes())));
    t.join(" ")
}

/// The cases every seed starts with: the help, the version options, the parse-only report, the
/// names by option, the combinations the checks refuse, the files the options write.
fn prelude() -> Vec<String> {
    let io: &[&str] = &["in.mgz", "out.mgz"];
    let with = |a: &[&str]| -> Vec<String> {
        a.iter().chain(io.iter()).map(|s| (*s).to_owned()).collect()
    };
    let mut v: Vec<Vec<String>> = vec![
        vec![],
        vec!["-u".into()],
        vec!["--usage".into()],
        vec!["--help".into()],
        vec!["-h".into(), "in.mgz".into()],
        vec!["-version2".into()],
        vec!["--version".into()],
        with(&["--version"]),
        with(&["-x"]),
        with(&["-po"]),
        with(&[
            "--parse_only", "-it", "mgz", "-ot", "nii", "-sn", "bert", "--invert_contrast", "2",
            "-r", "1", "2", "3", "-r4", "1", "2", "3", "4", "-nt",
        ]),
        vec!["-i".into(), "a.mgz".into(), "-o".into(), "b.mgz".into()],
        vec!["--input_volume".into(), "a.mgz".into(), "--output_volume".into(), "b.nii".into()],
        with(&["-dcm2niix", "-createBIDS", "-dcm2niix-outdir", "dir"]),
        with(&["-no-dcm2niix", "--createBIDS", "--dcm2niix-outdir", "dir"]),
        with(&["-dcm2niix-createBIDS"]),
        with(&["--antialias", "-vs", "1", "1", "1"]),
        with(&["--antialias"]),
        with(&["--antialias", "-vs", "1", "1", "1", "--fwhm", "2"]),
        vec!["--out_stats_table".into(), "--like".into(), "t.dat".into(), "in.dat".into(), "out.dat".into()],
        vec!["--out_stats_table".into(), "in.dat".into(), "out.dat".into()],
        vec!["--in_stats_table".into(), "in.dat".into(), "out.mgz".into()],
        with(&["--force_ras_good", "-iid", "1", "0", "0"]),
        with(&["-ds", "2", "2", "2", "-dsold", "2", "2", "2"]),
        with(&["-ro", "-oi"]),
        vec!["-ro".into(), "in.mgz".into()],
        vec!["-nw".into(), "in.mgz".into()],
        vec!["in.mgz".into(), "out".into()],
        vec!["-it".into(), "spm".into(), "in.img".into(), "out.mgz".into()],
        with(&["--nskip", "-1"]),
        with(&["--ndrop", "2", "--nskip", "3"]),
        with(&["--debug", "--debug"]),
        with(&["--statusfile", "st.txt"]),
        with(&["-f", "0", "1", "2", "-nt"]),
        with(&["-T", "x.lta", "--like", "lk.mgz", "-ini", "4", "--slice-reverse", "--scale", "2"]),
        with(&["-iid", "3", "4", "0", "-ojd", "0", "0", "0"]),
        with(&["-io", "lia", "-oo", "LLA"]),
        with(&["--out_matrix", "-ti", "-onk", "7", "--in_like", "x.mgz", "-rl", "y.mgz"]),
    ];
    let mut lines: Vec<String> = v
        .drain(..)
        .map(|a| {
            let a: Vec<&str> = a.iter().map(String::as_str).collect();
            opt_line(&[], &a)
        })
        .collect();
    let m: &[u8] = b"1 0 0 4\n0 1 0 -2\n0 0 1 3.5\n0 0 0 1\n";
    lines.push(opt_line(&[("in_m.txt", m)], &["--autoalign", "in_m.txt", "in.mgz", "out.mgz"]));
    lines.push(opt_line(
        &[("in_m.txt", m), ("in_n.txt", b"2 0\n0 2\n")],
        &["--left-right-swap-label-table", "in_m.txt", "in_n.txt", "in.mgz", "out.mgz"],
    ));
    lines.push(opt_line(&[("in_t.lta", b"x")], &["-ait", "in_t.lta", "in.mgz", "out.mgz"]));
    lines.push(opt_line(&[("in_l.txt", b"x")], &["--sdcmlist", "in_l.txt", "in.mgz", "out.mgz"]));
    lines
}

/// `n` case lines of seed `seed`: the [`prelude`], then seeded ones.
pub fn generate(seed: u64, n: usize) -> Vec<String> {
    let mut v = prelude();
    v.truncate(n);
    v.extend(generate_seeded(seed, n - v.len()));
    v
}

/// `n` seeded case lines.
fn generate_seeded(seed: u64, n: usize) -> Vec<String> {
    let mut g = Gen::new(seed);
    // every option name drawn alike
    let specs: Vec<(&'static str, Kind)> = options()
        .into_iter()
        .flat_map(|(names, kind)| names.iter().map(move |&n| (n, kind)))
        .collect();
    let mut v = Vec::with_capacity(n);
    for k in 0..n {
        let line = match k % 20 {
            0 => format!("csv {}", hex(&csv_text(&mut g))),
            1 => {
                let name = g.pick(&["in_t.dat", "in_t.dat", "in_t.stats", "in_tbl", "in_t.DAT", "in_t."]);
                format!("lst {name} {}", hex(&fs_table(&mut g)))
            }
            2 => {
                let name = g.pick(&["in_t.csv", "in_t.CSV", "in_t.csv"]);
                let text = if g.chance(4) { fs_table(&mut g) } else { csv_text(&mut g) };
                format!("lst {name} {}", hex(&text))
            }
            3 => {
                let tac = g.chance(2);
                let name = if tac {
                    g.pick(&["in_t.tac", "in_t.TAC"])
                } else {
                    g.pick(&["in_t.tsv", "in_t.Tsv"])
                };
                format!("lst {name} {}", hex(&tsv_text(&mut g, tac)))
            }
            4 => format!("lst {} -", g.pick(&["in_none.dat", "in_none.csv", "in_none.tsv", "in_none"])),
            5 | 6 => {
                let w = g.int(1, 4) as i32;
                let nf = g.int(1, 3) as i32;
                let vol = vol_tokens(&mut g, w, nf);
                let (t, h) = template(&mut g, w, nf);
                if k % 20 == 5 {
                    format!("ist {vol} {t} {h}")
                } else {
                    format!("wst {vol} {t} {h} {}", g.pick(&["out.txt", "out.txt", "nodir/out.txt"]))
                }
            }
            _ => opt_case(&mut g, &specs),
        };
        v.push(line);
    }
    v
}

// ------------------------------------------------------------------ the port's answers

#[derive(Clone, Default)]
struct Shared(Rc<RefCell<Vec<u8>>>);

impl Write for Shared {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct Toks<'a> {
    t: Vec<&'a str>,
    i: usize,
}

impl<'a> Toks<'a> {
    fn next(&mut self) -> &'a str {
        let s = self.t[self.i];
        self.i += 1;
        s
    }
    fn int(&mut self) -> i32 {
        self.next().parse().expect("an integer")
    }
    fn float(&mut self) -> f32 {
        f32::from_bits(u32::from_str_radix(self.next(), 16).expect("float bits"))
    }
}

/// `fsconvert`'s `oh`: `N` for `NULL`, `-` for an empty string, else hex.
fn oh(s: Option<&[u8]>) -> String {
    match s {
        None => "N".into(),
        Some([]) => "-".into(),
        Some(b) => b.iter().map(|c| format!("{c:02x}")).collect(),
    }
}

fn get_vol(g: &mut Globals, t: &mut Toks) -> Mri {
    let (w, h, d, ty, nf) = (t.int(), t.int(), t.int(), t.int(), t.int());
    let mut sink = Vec::new();
    let mut sink2 = Vec::new();
    let mut m = mri_alloc_sequence(g, &mut sink, &mut sink2, w, h, d, ty, nf).expect("a volume");
    m.vg.xsize = t.float();
    m.vg.ysize = t.float();
    m.vg.zsize = t.float();
    let n = (w * h * d * nf) as usize;
    let bpv = match ty {
        MRI_UCHAR => 1,
        MRI_SHORT => 2,
        _ => 4,
    };
    let mut bytes = Vec::with_capacity(n * bpv);
    while bytes.len() < n * bpv {
        let tok = t.next();
        let (v, k) = match tok.split_once('*') {
            Some((v, k)) => (v, k.parse::<usize>().expect("a count")),
            None => (tok, 1),
        };
        for _ in 0..k {
            match ty {
                MRI_UCHAR => bytes.push(v.parse::<i64>().expect("uchar") as u8),
                MRI_SHORT => bytes.extend((v.parse::<i64>().expect("short") as i16).to_le_bytes()),
                MRI_INT => bytes.extend((v.parse::<i64>().expect("int") as i32).to_le_bytes()),
                MRI_FLOAT => bytes.extend(u32::from_str_radix(v, 16).expect("float").to_le_bytes()),
                _ => panic!("a voxel type the cases do not write"),
            }
        }
    }
    bytes.truncate(n * bpv);
    if let Some(c) = m.chunk.as_mut() {
        c[..bytes.len()].copy_from_slice(&bytes);
    }
    m
}

fn put_vol(s: &mut String, m: Option<&Mri>) {
    let Some(m) = m else {
        *s += "NULL ";
        return;
    };
    let v = &m.vg;
    for x in [v.width, v.height, v.depth, m.mtype, m.nframes] {
        *s += &format!("{x} ");
    }
    *s += &format!("{:016x} ", m.outside_val.to_bits());
    for x in [
        v.xsize, v.ysize, v.zsize, v.x_r, v.x_a, v.x_s, v.y_r, v.y_a, v.y_s, v.z_r, v.z_a, v.z_s,
        v.c_r, v.c_a, v.c_s,
    ] {
        *s += &format!("{:08x} ", x.to_bits());
    }
    *s += &format!("{} ", v.ras_good_flag);
    let n = (v.width as usize) * (v.height as usize) * (v.depth as usize) * (m.nframes as usize);
    let chunk = m.chunk.as_deref().unwrap_or(&[]);
    let mut toks: Vec<String> = Vec::with_capacity(n);
    for i in 0..n {
        toks.push(match m.mtype {
            MRI_UCHAR => chunk[i].to_string(),
            MRI_SHORT => i16::from_le_bytes([chunk[2 * i], chunk[2 * i + 1]]).to_string(),
            MRI_INT => i32::from_le_bytes(chunk[4 * i..4 * i + 4].try_into().unwrap()).to_string(),
            MRI_FLOAT => format!(
                "{:08x}",
                u32::from_le_bytes(chunk[4 * i..4 * i + 4].try_into().unwrap())
            ),
            t => panic!("a voxel type the cases do not print: {t}"),
        });
    }
    let mut i = 0;
    while i < toks.len() {
        let mut k = 1;
        while i + k < toks.len() && toks[i + k] == toks[i] {
            k += 1;
        }
        *s += &toks[i];
        if k > 1 {
            *s += &format!("*{k}");
        }
        *s += " ";
        i += k;
    }
}

fn put_table(s: &mut String, st: Option<&StatTable>) {
    let Some(st) = st else {
        *s += "NULL ";
        return;
    };
    *s += "T ";
    *s += &oh(st.filename.as_deref().map(str::as_bytes));
    *s += " ";
    *s += &oh(st.measure.as_deref());
    *s += &format!(" {} {} ", st.nrows, st.ncols);
    for c in &st.colnames {
        *s += &oh(c.as_deref());
        *s += " ";
    }
    for r in &st.rownames {
        *s += &oh(r.as_deref());
        *s += " ";
    }
    for row in &st.data {
        for x in row {
            *s += &format!("{:016x} ", x.to_bits());
        }
    }
    put_vol(s, st.mri.as_ref());
}

fn fnv(b: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &c in b {
        h = (h ^ u64::from(c)).wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// The files a case wrote (names not starting with `in_`), as `fsconvert` prints them.
fn written(dir: &Path) -> String {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|d| {
            d.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| !n.starts_with("in_"))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    let mut s = String::new();
    for n in names {
        s += &format!("F {n} ");
        let path = dir.join(&n);
        if n.len() > 4 && n.ends_with(".mgz") {
            let mut g = Globals::new(UpstreamDefects::Reproduce);
            let env = Environment::default();
            let (mut a, mut b) = (Vec::new(), Vec::new());
            let m = mri_read_volume(&mut g, &mut a, &mut b, &env, Some(dir), &n)
                .ok()
                .flatten();
            put_vol(&mut s, m.as_ref());
        } else {
            let b = std::fs::read(&path).unwrap_or_default();
            s += &format!("{:016x} ", fnv(&b));
        }
    }
    s.trim_end().to_owned()
}

fn escape(b: &[u8]) -> String {
    let mut s = String::new();
    for &c in b {
        if !(0x20..=0x7e).contains(&c) || c == b'\\' {
            s += &format!("\\x{c:02x}");
        } else {
            s.push(c as char);
        }
    }
    s
}

// the dump of the probe's hook (`hook_dump.inc`), name by name
fn di(d: &mut String, n: &str, v: i64) {
    *d += &format!("{n}={v} ");
}
fn diu(d: &mut String, n: &str, v: Option<i32>) {
    match v {
        Some(v) => di(d, n, i64::from(v)),
        None => *d += &format!("{n}=U "),
    }
}
fn dia(d: &mut String, n: &str, v: Option<&[i32]>, k: usize) {
    *d += &format!("{n}=");
    for j in 0..k {
        if j > 0 {
            *d += ",";
        }
        match v {
            Some(v) => *d += &v[j].to_string(),
            None => *d += "U",
        }
    }
    *d += " ";
}
fn df(d: &mut String, n: &str, v: Option<f32>) {
    match v {
        Some(v) => *d += &format!("{n}={:08x} ", v.to_bits()),
        None => *d += &format!("{n}=U "),
    }
}
fn dfa(d: &mut String, n: &str, v: Option<[f32; 3]>) {
    *d += &format!("{n}=");
    for j in 0..3 {
        if j > 0 {
            *d += ",";
        }
        match v {
            Some(v) => *d += &format!("{:08x}", v[j].to_bits()),
            None => *d += "U",
        }
    }
    *d += " ";
}
fn hexs(s: &[u8]) -> String {
    let s = &s[..s.iter().position(|&c| c == 0).unwrap_or(s.len())];
    s.iter().map(|c| format!("{c:02x}")).collect()
}
fn ds<S: AsRef<str>>(d: &mut String, n: &str, v: Option<S>) {
    match v {
        Some(v) => *d += &format!("{n}={} ", hexs(v.as_ref().as_bytes())),
        None => *d += &format!("{n}=U "),
    }
}
fn dp(d: &mut String, n: &str, v: Option<&str>) {
    match v {
        Some(v) => *d += &format!("{n}={} ", hexs(v.as_bytes())),
        None => *d += &format!("{n}=N "),
    }
}
fn dm(d: &mut String, n: &str, m: Option<&Matrix>) {
    let Some(m) = m else {
        *d += &format!("{n}=N ");
        return;
    };
    *d += &format!("{n}={}x{}:", m.rows, m.cols);
    let mut first = true;
    for r in 1..=m.rows {
        for c in 1..=m.cols {
            if !first {
                *d += ",";
            }
            first = false;
            *d += &format!("{:08x}", m.relt(r, c).to_bits());
        }
    }
    *d += " ";
}

/// The dump of `hook_dump.inc` for the port's options.
pub fn dump(o: &Options, g: &Globals, env: &Environment) -> String {
    let mut d = String::new();
    let s = |x: &str| Some(x.to_owned());
    let fl = |x: f32| Some(x);
    di(&mut d, "outside_val", o.outside_val.into());
    di(&mut d, "reorder_flag", o.reorder_flag.into());
    dia(&mut d, "reorder_vals", o.reorder_vals.as_ref().map(|v| &v[..]), 3);
    di(&mut d, "reorder4_flag", o.reorder4_flag.into());
    dia(&mut d, "reorder4_vals", o.reorder4_vals.as_ref().map(|v| &v[..]), 4);
    di(&mut d, "shiftdim_flag", o.shiftdim_flag.into());
    dia(&mut d, "shift_dim", o.shift_dim.as_ref().map(|v| &v[..]), 3);
    df(&mut d, "invert_val", fl(o.invert_val));
    ds(&mut d, "in_name", s(&o.in_name));
    ds(&mut d, "out_name", s(&o.out_name));
    di(&mut d, "conform_flag", o.conform_flag.into());
    di(&mut d, "ConfKeepDC", o.conf_keep_dc.into());
    di(&mut d, "conform_width_256_flag", o.conform_width_256_flag.into());
    di(&mut d, "conform_min", o.conform_min.into());
    df(&mut d, "conform_size", fl(o.conform_size));
    ds(&mut d, "AutoAlignFile", o.auto_align_file.as_deref());
    dm(&mut d, "AutoAlign", o.auto_align.as_ref());
    for (n, v) in [
        ("nochange_flag", o.nochange_flag),
        ("store_orig_ras2vox_flag", o.store_orig_ras2vox_flag),
        ("parse_only_flag", o.parse_only_flag),
        ("in_info_flag", o.in_info_flag),
        ("out_info_flag", o.out_info_flag),
        ("template_info_flag", o.template_info_flag),
        ("in_stats_flag", o.in_stats_flag),
        ("out_stats_flag", o.out_stats_flag),
        ("read_only_flag", o.read_only_flag),
        ("no_write_flag", o.no_write_flag),
        ("in_matrix_flag", o.in_matrix_flag),
        ("out_matrix_flag", o.out_matrix_flag),
        ("force_ras_good", o.force_ras_good),
        ("sphinx_flag", o.sphinx_flag),
    ] {
        di(&mut d, n, v.into());
    }
    ds(&mut d, "transform_fname", o.transform_fname.as_deref());
    di(&mut d, "transform_flag", o.transform_flag.into());
    di(&mut d, "invert_transform_flag", o.invert_transform_flag.into());
    ds(&mut d, "out_like_name", o.out_like_name.as_deref());
    di(&mut d, "out_like_flag", o.out_like_flag.into());
    di(&mut d, "crop_flag", o.crop_flag.into());
    dia(&mut d, "crop_center", Some(&o.crop_center[..]), 3);
    dia(&mut d, "crop_size", Some(&o.crop_size[..]), 3);
    ds(&mut d, "devxfm_subject", o.devxfm_subject.as_deref());
    di(&mut d, "DevXFM", o.dev_xfm.into());
    di(&mut d, "UpsampleFlag", o.upsample_flag.into());
    di(&mut d, "UpsampleFactor", o.upsample_factor.into());
    for (k, a) in ["i", "j", "k"].iter().enumerate() {
        di(&mut d, &format!("in_{a}_size_flag"), o.in_size_flag[k].into());
        df(&mut d, &format!("in_{a}_size"), o.in_size[k]);
    }
    for (k, a) in ["i", "j", "k"].iter().enumerate() {
        di(&mut d, &format!("out_{a}_size_flag"), o.out_size_flag[k].into());
        df(&mut d, &format!("out_{a}_size"), o.out_size[k]);
    }
    for (k, a) in ["i", "j", "k"].iter().enumerate() {
        di(&mut d, &format!("in_{a}_direction_flag"), o.in_direction_flag[k].into());
        dfa(&mut d, &format!("in_{a}_directions"), o.in_directions[k]);
    }
    for (k, a) in ["i", "j", "k"].iter().enumerate() {
        di(&mut d, &format!("out_{a}_direction_flag"), o.out_direction_flag[k].into());
        dfa(&mut d, &format!("out_{a}_directions"), o.out_directions[k]);
    }
    ds(&mut d, "colortablefile", s(&o.colortablefile));
    di(&mut d, "in_orientation_flag", o.in_orientation_flag.into());
    ds(&mut d, "in_orientation_string", o.in_orientation_string.as_deref());
    di(&mut d, "out_orientation_flag", o.out_orientation_flag.into());
    ds(&mut d, "out_orientation_string", o.out_orientation_string.as_deref());
    di(&mut d, "in_center_flag", o.in_center_flag.into());
    dfa(&mut d, "in_center", o.in_center);
    di(&mut d, "delta_in_center_flag", o.delta_in_center_flag.into());
    dfa(&mut d, "delta_in_center", o.delta_in_center);
    di(&mut d, "out_center_flag", o.out_center_flag.into());
    dfa(&mut d, "out_center", o.out_center);
    for (k, a) in ["i", "j", "k"].iter().enumerate() {
        di(&mut d, &format!("out_n_{a}_flag"), o.out_n_flag[k].into());
        diu(&mut d, &format!("out_n_{a}"), o.out_n[k]);
    }
    di(&mut d, "voxel_size_flag", o.voxel_size_flag.into());
    dfa(&mut d, "voxel_size", o.voxel_size);
    di(&mut d, "downsample_flag", o.downsample_flag.into());
    di(&mut d, "downsampleold_flag", o.downsampleold_flag.into());
    di(&mut d, "downsample2_flag", o.downsample2_flag.into());
    dfa(&mut d, "downsample_factor", o.downsample_factor);
    for (k, a) in ["i", "j", "k"].iter().enumerate() {
        di(&mut d, &format!("in_n_{a}_flag"), o.in_n_flag[k].into());
        diu(&mut d, &format!("in_n_{a}"), o.in_n[k]);
    }
    di(&mut d, "antialias_flag", o.antialias_flag.into());
    di(&mut d, "nthreads", o.nthreads.into());
    df(&mut d, "fwhm", fl(o.fwhm));
    df(&mut d, "gstd", fl(o.gstd));
    di(&mut d, "reduce", o.reduce.into());
    di(&mut d, "in_tr_flag", o.in_tr_flag.into());
    df(&mut d, "in_tr", fl(o.in_tr));
    di(&mut d, "in_ti_flag", o.in_ti_flag.into());
    df(&mut d, "in_ti", fl(o.in_ti));
    di(&mut d, "in_te_flag", o.in_te_flag.into());
    df(&mut d, "in_te", fl(o.in_te));
    di(&mut d, "in_flip_angle_flag", o.in_flip_angle_flag.into());
    df(&mut d, "in_flip_angle", fl(o.in_flip_angle));
    di(&mut d, "out_data_type", o.out_data_type.into());
    ds(&mut d, "out_data_type_string", o.out_data_type_string.as_deref());
    df(&mut d, "rescale_factor", fl(o.rescale_factor));
    df(&mut d, "scale_factor", fl(o.scale_factor));
    df(&mut d, "out_scale_factor", fl(o.out_scale_factor));
    di(&mut d, "rescale_voxel", o.rescale_voxel.into());
    for (k, a) in ["c", "r", "s"].iter().enumerate() {
        diu(&mut d, &format!("rescale_voxel_{a}"), o.rescale_voxel_crs[k]);
    }
    ds(&mut d, "resample_type", o.resample_type.as_deref());
    di(&mut d, "resample_type_val", o.resample_type_val.into());
    di(&mut d, "force_in_type_flag", o.force_in_type_flag.into());
    di(&mut d, "forced_in_type", o.forced_in_type.into());
    ds(&mut d, "in_type_string", s(&o.in_type_string));
    di(&mut d, "force_out_type_flag", o.force_out_type_flag.into());
    di(&mut d, "forced_out_type", o.forced_out_type.into());
    ds(&mut d, "out_type_string", o.out_type_string.as_deref());
    di(&mut d, "force_template_type_flag", o.force_template_type_flag.into());
    di(&mut d, "forced_template_type", o.forced_template_type.into());
    ds(&mut d, "template_type_string", o.template_type_string.as_deref());
    ds(&mut d, "subject_name", s(&o.subject_name));
    ds(&mut d, "gdf_image_stem", s(&o.gdf_image_stem));
    di(&mut d, "reslice_like_flag", o.reslice_like_flag.into());
    ds(&mut d, "reslice_like_name", o.reslice_like_name.as_deref());
    di(&mut d, "frame_flag", o.frame_flag.into());
    di(&mut d, "mid_frame_flag", o.mid_frame_flag.into());
    di(&mut d, "nframes", o.nframes.into());
    dia(&mut d, "frames", Some(&o.frames[..]), o.frames.len());
    di(&mut d, "erode_seg_flag", o.erode_seg_flag.into());
    diu(&mut d, "n_erode_seg", o.n_erode_seg);
    di(&mut d, "dil_seg_flag", o.dil_seg_flag.into());
    diu(&mut d, "n_dil_seg", o.n_dil_seg);
    ds(&mut d, "dil_seg_mask", s(&o.dil_seg_mask));
    di(&mut d, "SliceBias", o.slice_bias.into());
    df(&mut d, "SliceBiasAlpha", fl(o.slice_bias_alpha));
    di(&mut d, "subsample_flag", o.subsample_flag.into());
    for (k, a) in ["SubSampStart", "SubSampDelta", "SubSampEnd"].iter().enumerate() {
        diu(&mut d, a, o.sub_samp[k]);
    }
    di(&mut d, "in_like_flag", o.in_like_flag.into());
    ds(&mut d, "in_like_name", o.in_like_name.as_deref());
    di(&mut d, "FirstDicom", o.first_dicom.into());
    ds(&mut d, "FirstDicomFile", o.first_dicom_file.as_deref());
    di(&mut d, "roi_flag", o.roi_flag.into());
    di(&mut d, "fill_parcellation_flag", o.fill_parcellation_flag.into());
    di(&mut d, "zero_outlines_flag", o.zero_outlines_flag.into());
    di(&mut d, "smooth_parcellation_flag", o.smooth_parcellation_flag.into());
    diu(&mut d, "smooth_parcellation_count", o.smooth_parcellation_count);
    di(&mut d, "color_file_flag", o.color_file_flag.into());
    ds(&mut d, "color_file_name", o.color_file_name.as_deref());
    di(&mut d, "translate_labels_flag", o.translate_labels_flag.into());
    di(&mut d, "no_scale_flag", o.no_scale_flag.into());
    di(&mut d, "nthframe", o.nthframe.into());
    di(&mut d, "LeftRightReverse", o.left_right_reverse.into());
    di(&mut d, "LeftRightReversePix", o.left_right_reverse_pix.into());
    di(&mut d, "LeftRightMirrorFlag", o.left_right_mirror_flag.into());
    di(&mut d, "LeftRightKeepFlag", o.left_right_keep_flag.into());
    ds(&mut d, "LeftRightMirrorHemi", o.left_right_mirror_hemi.as_deref());
    di(&mut d, "LeftRightSwapLabel", o.left_right_swap_label.into());
    dm(&mut d, "lhswap", o.lhswap.as_ref());
    dm(&mut d, "rhswap", o.rhswap.as_ref());
    for (n, v) in [
        ("FlipCols", o.flip_cols),
        ("SliceReverse", o.slice_reverse),
        ("ascii_flag", o.ascii_flag),
        ("InStatTableFlag", o.in_stat_table_flag),
        ("OutStatTableFlag", o.out_stat_table_flag),
        ("zero_ge_z_offset_flag", o.zero_ge_z_offset_flag),
        ("nskip", o.nskip),
        ("ndrop", o.ndrop),
    ] {
        di(&mut d, n, v.into());
    }
    ds(&mut d, "in_name_only", o.in_name_only.as_deref());
    di(&mut d, "out_volume_type", o.out_volume_type.into());
    di(&mut d, "debug", o.debug.into());
    df(&mut d, "upper_thresh", fl(o.upper_thresh));
    for (n, v) in [
        ("upper_thresh_flag", o.upper_thresh_flag),
        ("ncutends", o.ncutends),
        ("cutends_flag", o.cutends_flag),
        ("slice_crop_flag", o.slice_crop_flag),
        ("slice_crop_start", o.slice_crop_start),
        ("slice_crop_stop", o.slice_crop_stop),
        ("SplitFrames", o.split_frames),
        ("DeleteCMDs", o.delete_cmds),
        ("DoNewTransformFname", o.do_new_transform_fname),
    ] {
        di(&mut d, n, v.into());
    }
    ds(&mut d, "NewTransformFname", s(&o.new_transform_fname));
    let x = &o.dicom;
    di(&mut d, "UseDCM2NIIX", x.use_dcm2niix.into());
    di(&mut d, "DCM2NIIX_createBIDS", x.dcm2niix_create_bids.into());
    dp(&mut d, "DCM2NIIX_outdir", x.dcm2niix_outdir.as_deref());
    di(&mut d, "DCM2NIIX_no_ForceStackSameSeries", x.dcm2niix_no_force_stack_same_series.into());
    dp(&mut d, "DCM2NIIX_INFO_DUMP", x.dcm2niix_info_dump.as_deref());
    dp(&mut d, "DCM2NIIX_DICOM_FLIST", x.dcm2niix_dicom_flist.as_deref());
    di(&mut d, "DCM2NIIX_OnlySingleFile", x.dcm2niix_only_single_file.into());
    dp(&mut d, "DCM2NIIX_OPTS", x.dcm2niix_opts.as_deref());
    di(&mut d, "DoSiemensAsciiDump", x.do_siemens_ascii_dump.into());
    di(&mut d, "DoSiemensAsciiAltDump", x.do_siemens_ascii_alt_dump.into());
    di(&mut d, "UseDICOMRead2", x.use_dicom_read2.into());
    dp(&mut d, "SDCMStatusFile", x.sdcm_status_file.as_deref());
    dp(&mut d, "SDCMListFile", x.sdcm_list_file.as_deref());
    di(&mut d, "N_Zero_Pad_Output", g.n_zero_pad_output.into());
    di(&mut d, "N_Zero_Pad_Input", g.n_zero_pad_input.into());
    di(&mut d, "MRIIO_Strip_Pound", g.mriio_strip_pound.into());
    di(&mut d, "Gdiag_no", g.gdiag_no.into());
    di(&mut d, "Gdiag", g.gdiag as i64);
    di(&mut d, "SliceResElTag1", x.slice_res_el_tag1);
    di(&mut d, "SliceResElTag2", x.slice_res_el_tag2);
    di(&mut d, "AutoSliceResElTag", x.auto_slice_res_el_tag.into());
    di(
        &mut d,
        "DICOMReadFirstDicomFile[0]",
        x.first_dicom_file.first().map_or(0, |&c| i64::from(c as i8)),
    );
    for name in [
        "FS_LOAD_DWI",
        "FS_RESCALE_DICOM",
        "FS_DESIRED_BVEC_SPACE",
        "FS_ANALYZE_NO_RESCALE",
        "BFILE_LITTLE_ENDIAN",
        "FS_dcmGetDWIParamsSiemens_VoxelSpace",
        "NSLICES_OVERRIDE",
        "NCOLS_OVERRIDE",
        "NROWS_OVERRIDE",
        "FS_MOSAIC_FIX_NOASCII",
    ] {
        match env.get(name) {
            Some(v) => d += &format!("{name}={} ", hexs(v.as_encoded_bytes())),
            None => d += &format!("{name}=N "),
        }
    }
    d.trim_end().to_owned()
}

/// The environment `check.py` gives the official probe, with the probe's own variable.
pub fn probe_env() -> Environment {
    Environment::from_pairs([
        ("PATH", "/usr/bin:/bin"),
        ("LC_ALL", "C"),
        ("LANG", "C"),
        ("OMP_NUM_THREADS", "1"),
        ("SURFER_SIDEDOOR", "1"),
    ])
}

fn write_case_file(dir: &Path, name: &str, h: &str) {
    std::fs::write(dir.join(name), unhex(h)).expect("a case file");
}

/// The port's line for a case line, and the defects it noted (`dir`: an empty folder for the
/// case's files).
pub fn run_line_in(line: &str, mode: UpstreamDefects, dir: &Path) -> (String, Vec<&'static str>) {
    let shared = Shared::default();
    let mut g = Globals::new(mode);
    g.progname = "mri_convert".to_owned();
    g.comm = "fsconvert".to_owned();
    let mut t = Toks {
        t: line.split_whitespace().collect(),
        i: 0,
    };
    let mut env = probe_env();
    let mut res = String::new();
    let mut a = shared.clone();
    let mut b = shared.clone();
    let cmd = t.next();
    let outcome: Result<(), Fatal> = (|| {
        match cmd {
            "csv" => {
                let bytes = unhex(t.next());
                write_case_file(dir, "in_t.csv", &hex(&bytes));
                let tbl = read_csv(&bytes);
                res += &format!("{} ", tbl.len());
                for row in &tbl {
                    res += &format!("{} ", row.len());
                    for f in row {
                        res += &oh(Some(&f[..f.iter().position(|&c| c == 0).unwrap_or(f.len())]));
                        res += " ";
                    }
                }
            }
            "lst" => {
                let name = t.next();
                let h = t.next();
                if h != "-" {
                    write_case_file(dir, name, h);
                }
                let mut io = Io {
                    out: &mut a,
                    err: &mut b,
                    env: &env,
                    dir: Some(dir),
                };
                let st = load_stat_table(&mut g, &mut io, name)?;
                put_table(&mut res, st.as_ref());
            }
            "ist" | "wst" => {
                let m = get_vol(&mut g, &mut t);
                let name = t.next().to_owned();
                let h = t.next();
                let tmpl: Option<&str> = match name.as_str() {
                    "-" => None,
                    "@" => Some(""),
                    n => {
                        if h != "-" {
                            write_case_file(dir, n, h);
                        }
                        Some(n)
                    }
                };
                let mut io = Io {
                    out: &mut a,
                    err: &mut b,
                    env: &env,
                    dir: Some(dir),
                };
                let st = init_stat_table_from_mri(&mut g, &mut io, &m, tmpl)?;
                put_table(&mut res, st.as_ref());
                if cmd == "wst" {
                    let o = t.next();
                    if let Some(st) = &st {
                        write_stat_table(&mut g, &mut io, o, st)?;
                    }
                }
            }
            "opt" => {
                let nfiles = t.int();
                for _ in 0..nfiles {
                    let f = t.next();
                    let h = t.next();
                    write_case_file(dir, f, h);
                }
                let n = t.int() as usize;
                let mut argv = vec!["mri_convert".to_owned()];
                for _ in 0..n {
                    argv.push(String::from_utf8(unhex(t.next())).expect("an argument"));
                }
                let mut r = Run {
                    out: &mut a,
                    err: &mut b,
                    env: &mut env,
                    dir: Some(dir),
                };
                let o = parse(&mut g, &mut r, &argv)?;
                res = dump(&o, &g, &env);
                res += " ";
            }
            c => panic!("unknown command {c}"),
        }
        Ok(())
    })();
    let text = shared.0.borrow().clone();
    let mut result = match outcome {
        Ok(()) => res.trim_end().to_owned(),
        Err(f) => format!("EXIT {}", f.status()),
    };
    let files = written(dir);
    if !files.is_empty() {
        if !result.is_empty() {
            result.push(' ');
        }
        result += &files;
    }
    let ids = g.notices.as_slice().iter().map(|n| n.defect.id).collect();
    (format!("{result} | {}", escape(&text)), ids)
}

/// [`run_line_in`] in a fresh temporary folder.
pub fn run_line(line: &str, mode: UpstreamDefects) -> (String, Vec<&'static str>) {
    let dir = tempfile::tempdir().expect("a temporary folder");
    run_line_in(line, mode, dir.path())
}
