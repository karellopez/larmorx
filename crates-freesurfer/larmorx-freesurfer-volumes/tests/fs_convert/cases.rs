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
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_INT, MRI_SHORT, MRI_UCHAR, Mri, mri_alloc_sequence,
};
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
    let w = [
        "Left-Hippo",
        "x",
        "Volume_mm3",
        "a\"b",
        "c,d",
        "lh",
        "rh.thickness",
        "BrainSeg",
    ];
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
        g.pick(&[
            "name",
            "\"quoted,field\"",
            "a\"\"b",
            "\"x\"y",
            "",
            " sp ",
            "z\r",
        ])
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
            .pick(&[
                "",
                " 5",
                "+4",
                "12x",
                "2147483648",
                "99999999999999999999",
                "0x10",
                "1e3",
                "-",
            ])
            .to_owned();
    }
    g.int(-3, 30).to_string()
}

/// An argument of an option taking numbers.
fn arg_float(g: &mut Gen) -> String {
    if g.chance(5) {
        return g.pick(NUMBERS).to_owned();
    }
    g.pick(&[
        "1", "0.5", "-2", "3.75", "0", "1.25", "256", "0.001", "1e-3", "",
    ])
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
    "--debug",
    "--left-right-reverse",
    "--left-right-reverse-pix",
    "--left-right-swap-label",
    "--flip-cols",
    "--slice-reverse",
    "--ascii",
    "--ascii+crsf",
    "--ascii-fcol",
    "--in_stats_table",
    "--out_stats_table",
    "-c",
    "--conform",
    "--conform-dc",
    "--cw256",
    "--delete-cmds",
    "--sphinx",
    "--no-rescale-dicom",
    "--bvec-scanner",
    "--bvec-voxel",
    "--no-analyze-rescale",
    "-nc",
    "--nochange",
    "-so",
    "--store_orig_ras2vox",
    "-cm",
    "--conform_min",
    "-po",
    "--parse_only",
    "-ii",
    "--in_info",
    "-oi",
    "--out_info",
    "-ti",
    "--template_info",
    "-is",
    "--in_stats",
    "-os",
    "--out_stats",
    "-ro",
    "--read_only",
    "-nw",
    "--no_write",
    "-im",
    "--in_matrix",
    "-om",
    "--out_matrix",
    "--force_ras_good",
    "--split",
    "--antialias",
    "-ds2",
    "--downsample2",
    "--bfile-little-endian",
    "-dicomread3",
    "-dcm2niix",
    "--dcm2niix",
    "-no-dcm2niix",
    "--no-dcm2niix",
    "-createBIDS",
    "--createBIDS",
    "-dcm2niix-createBIDS",
    "--dcm2niix-createBIDS",
    "-dcm2niix-no-ForceStackSameSeries",
    "--dcm2niix-no-ForceStackSameSeries",
    "-dcm2niix-singlefile",
    "--dcm2niix-singlefile",
    "-siemens-ascii-dump",
    "-siemens-ascii-alt-dump",
    "-dicomread2",
    "-dicomread0",
    "-siemensBVecsCross",
    "-no-siemensBVecsCross",
    "--mid-frame",
    "-roi",
    "--roi",
    "-fp",
    "--fill_parcellation",
    "-zo",
    "--zero_outlines",
    "-nt",
    "--no_translate",
    "-cg",
    "--crop_gdf",
    "--mosaic-fix-noascii",
    "--no-strip-pound",
    "-zgez",
    "--zero_ge_z_offset",
    "-nozgez",
    "--no_zero_ge_z_offset",
    "--diag-debug",
    "--mra",
    "--auto-slice-res",
    "--no-dwi",
    "--no-ctab",
    "--remove-ctab",
];

const TYPES: &[&str] = &[
    "mgz", "nii", "MGZ", "mgh", "spm", "analyze", "cor", "gdf", "otl", "bogus",
];

fn options() -> Vec<(&'static [&'static str], Kind)> {
    vec![
        (
            &[
                "-oval",
                "--outside_val",
                "--upsample",
                "--nthreads",
                "--threads",
                "--reduce",
            ],
            Kind::Ints(1),
        ),
        (
            &[
                "-oni",
                "-oic",
                "--out_i_count",
                "-onj",
                "-ojc",
                "--out_j_count",
                "-onk",
                "-okc",
                "--out_k_count",
            ],
            Kind::Ints(1),
        ),
        (
            &[
                "-ini",
                "-iic",
                "--in_i_count",
                "-inj",
                "-ijc",
                "--in_j_count",
                "-ink",
                "-ikc",
                "--in_k_count",
            ],
            Kind::Ints(1),
        ),
        (
            &[
                "--erode-seg",
                "--dil-seg",
                "--cutends",
                "-ns",
                "--no_scale",
                "-nth",
                "--nth_frame",
            ],
            Kind::Ints(1),
        ),
        (
            &["-sp", "--smooth_parcellation"],
            Kind::Pick(3, &["14", "26", "20", "13", "27"]),
        ),
        (
            &[
                "--nslices-override",
                "--ncols-override",
                "--nrows-override",
                "--nskip",
                "--ndrop",
                "--diag",
            ],
            Kind::Ints(1),
        ),
        (
            &["-r", "--reorder", "--shift", "--crop", "--cropsize"],
            Kind::Ints(3),
        ),
        (&["-r4", "--reorder4"], Kind::Ints(4)),
        (&["--slice-crop"], Kind::Ints(2)),
        (&["--fsubsample"], Kind::Ints(3)),
        (
            &[
                "-ut",
                "--upper_thresh",
                "--invert_contrast",
                "-cs",
                "--conform_size",
                "--fwhm",
                "-tr",
                "-TI",
                "-te",
                "-flip_angle",
                "--slice-bias",
            ],
            Kind::Floats(1),
        ),
        (
            &[
                "-iis",
                "--in_i_size",
                "-ijs",
                "--in_j_size",
                "-iks",
                "--in_k_size",
                "-ois",
                "--out_i_size",
                "-ojs",
                "--out_j_size",
                "-oks",
                "--out_k_size",
            ],
            Kind::Floats(1),
        ),
        (
            &[
                "-iid",
                "--in_i_direction",
                "-ijd",
                "--in_j_direction",
                "-ikd",
                "--in_k_direction",
                "-oid",
                "--out_i_direction",
                "-ojd",
                "--out_j_direction",
                "-okd",
                "--out_k_direction",
            ],
            Kind::Floats(3),
        ),
        (
            &[
                "-ic",
                "--in_center",
                "-dic",
                "--delta_in_center",
                "-oc",
                "--out_center",
                "-vs",
                "--voxsize",
                "-voxsize",
                "-ds",
                "--downsample",
                "-downsample",
                "-dsold",
                "--downsampleold",
                "-downsampleold",
            ],
            Kind::Floats(3),
        ),
        (
            &["--rescale", "-sc", "--scale", "-osc", "--out-scale"],
            Kind::Atof,
        ),
        (&["--rescale-voxel"], Kind::Scan(3)),
        (
            &["--nspmzeropad", "--out_nspmzeropad", "--in_nspmzeropad"],
            Kind::Scan(1),
        ),
        (
            &[
                "-i",
                "--input_volume",
                "-o",
                "--output_volume",
                "--new-transform-fname",
                "-at",
                "--apply_transform",
                "-T",
                "--like",
                "--devolvexfm",
                "--ctab",
                "-sn",
                "--subject_name",
                "-gis",
                "--gdf_image_stem",
                "-rl",
                "--reslice_like",
                "--dil-seg-mask",
                "-il",
                "--in_like",
                "--first-dicom",
                "-cf",
                "--color_file",
            ],
            Kind::Str,
        ),
        (
            &[
                "-dcm2niix-outdir",
                "--dcm2niix-outdir",
                "-dcm2niix-info-dump",
                "--dcm2niix-info-dump",
                "-dcm2niix-dicom-flist",
                "--dcm2niix-dicom-flist",
                "-dcm2niix-opts",
                "--dcm2niix-opts",
            ],
            Kind::Raw,
        ),
        (
            &["-odt", "--out_data_type"],
            Kind::Pick(5, &["uchar", "SHORT", "int", "Float", "rgb", "double", ""]),
        ),
        (
            &["-rt", "--resample_type"],
            Kind::Pick(
                5,
                &[
                    "interpolate",
                    "NEAREST",
                    "vote",
                    "weighted",
                    "cubic",
                    "sinc",
                    "bad",
                ],
            ),
        ),
        (
            &[
                "-it",
                "--in_type",
                "-ot",
                "--out_type",
                "-tt",
                "--template_type",
            ],
            Kind::Pick(4, TYPES),
        ),
        (
            &["-io", "--in_orientation", "-oo", "--out_orientation"],
            Kind::Pick(
                5,
                &[
                    "LIA", "RAS", "lia", "PSL", "LIAS", "LLA", "xyz", "LI", "R1A",
                ],
            ),
        ),
        (
            &["--left-right-mirror", "--left-right-keep"],
            Kind::Pick(2, &["lh", "rh", "LH", "both"]),
        ),
        (&["--left-right-swap-label-table"], Kind::Matrix2),
        (&["--autoalign"], Kind::Matrix),
        (&["-ait", "--apply_inverse_transform"], Kind::Exists),
        (&["--status", "--statusfile"], Kind::Write),
        (&["--sdcmlist"], Kind::Read),
        (&["-f", "--frame"], Kind::Frames),
        (
            &[
                "-u",
                "--usage",
                "--help",
                "-h",
                "-version2",
                "--version",
                "--bogus",
                "-x",
            ],
            Kind::Rare,
        ),
        (FLAGS, Kind::Flag),
    ]
}

const OUT_NAMES: &[&str] = &[
    "out.mgz",
    "out.mgz",
    "out.nii.gz",
    "out.nii",
    "out.mnc",
    "out.img",
    "out.xyz",
    "outname",
    "out.mgh",
    "out.gdf",
    "COR-",
];

const VALID_OUT: &[&str] = &[
    "out.mgz",
    "out.nii.gz",
    "out.nii",
    "out.mnc",
    "out.img",
    "out.mgh",
];

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
        g.pick(&[
            "1", "0.5", "-2", "3.75", "1.25", "256", "1e-3", "2.", ".5", "nan", "inf",
        ])
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
                        g.pick(&["name", "sub01", "a b", "in_x.lta", "lh.mgz"])
                            .to_owned()
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
        let (i, o) = (
            given(&["-i", "--input_volume"]),
            given(&["-o", "--output_volume"]),
        );
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
            2 => vec![
                "in.mgz".into(),
                g.pick(OUT_NAMES).into(),
                "extra.mgz".into(),
            ],
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
            "--parse_only",
            "-it",
            "mgz",
            "-ot",
            "nii",
            "-sn",
            "bert",
            "--invert_contrast",
            "2",
            "-r",
            "1",
            "2",
            "3",
            "-r4",
            "1",
            "2",
            "3",
            "4",
            "-nt",
        ]),
        vec!["-i".into(), "a.mgz".into(), "-o".into(), "b.mgz".into()],
        vec![
            "--input_volume".into(),
            "a.mgz".into(),
            "--output_volume".into(),
            "b.nii".into(),
        ],
        with(&["-dcm2niix", "-createBIDS", "-dcm2niix-outdir", "dir"]),
        with(&["-no-dcm2niix", "--createBIDS", "--dcm2niix-outdir", "dir"]),
        with(&["-dcm2niix-createBIDS"]),
        with(&["--antialias", "-vs", "1", "1", "1"]),
        with(&["--antialias"]),
        with(&["--antialias", "-vs", "1", "1", "1", "--fwhm", "2"]),
        vec![
            "--out_stats_table".into(),
            "--like".into(),
            "t.dat".into(),
            "in.dat".into(),
            "out.dat".into(),
        ],
        vec![
            "--out_stats_table".into(),
            "in.dat".into(),
            "out.dat".into(),
        ],
        vec!["--in_stats_table".into(), "in.dat".into(), "out.mgz".into()],
        with(&["--force_ras_good", "-iid", "1", "0", "0"]),
        with(&["-ds", "2", "2", "2", "-dsold", "2", "2", "2"]),
        with(&["-ro", "-oi"]),
        vec!["-ro".into(), "in.mgz".into()],
        vec!["-nw".into(), "in.mgz".into()],
        vec!["in.mgz".into(), "out".into()],
        vec![
            "-it".into(),
            "spm".into(),
            "in.img".into(),
            "out.mgz".into(),
        ],
        with(&["--nskip", "-1"]),
        with(&["--ndrop", "2", "--nskip", "3"]),
        with(&["--debug", "--debug"]),
        with(&["--statusfile", "st.txt"]),
        with(&["-f", "0", "1", "2", "-nt"]),
        with(&[
            "-T",
            "x.lta",
            "--like",
            "lk.mgz",
            "-ini",
            "4",
            "--slice-reverse",
            "--scale",
            "2",
        ]),
        with(&["-iid", "3", "4", "0", "-ojd", "0", "0", "0"]),
        // the NaNs of the direction's length: two NaN squares, a NaN from infinities
        with(&[
            "-iid", "nan", "-nan", "1", "-ijd", "-nan", "nan", "2", "-ikd", "inf", "0", "0",
        ]),
        with(&[
            "-oid", "1", "nan", "-nan", "-ojd", "0", "-inf", "inf", "-okd", "-nan", "-nan", "3",
        ]),
        with(&["--fwhm", "-nan", "-ut", "nan"]),
        with(&["-io", "lia", "-oo", "LLA"]),
        with(&[
            "--out_matrix",
            "-ti",
            "-onk",
            "7",
            "--in_like",
            "x.mgz",
            "-rl",
            "y.mgz",
        ]),
    ];
    let mut lines: Vec<String> = v
        .drain(..)
        .map(|a| {
            let a: Vec<&str> = a.iter().map(String::as_str).collect();
            opt_line(&[], &a)
        })
        .collect();
    let m: &[u8] = b"1 0 0 4\n0 1 0 -2\n0 0 1 3.5\n0 0 0 1\n";
    lines.push(opt_line(
        &[("in_m.txt", m)],
        &["--autoalign", "in_m.txt", "in.mgz", "out.mgz"],
    ));
    lines.push(opt_line(
        &[("in_m.txt", m), ("in_n.txt", b"2 0\n0 2\n")],
        &[
            "--left-right-swap-label-table",
            "in_m.txt",
            "in_n.txt",
            "in.mgz",
            "out.mgz",
        ],
    ));
    lines.push(opt_line(
        &[("in_t.lta", b"x")],
        &["-ait", "in_t.lta", "in.mgz", "out.mgz"],
    ));
    lines.push(opt_line(
        &[("in_l.txt", b"x")],
        &["--sdcmlist", "in_l.txt", "in.mgz", "out.mgz"],
    ));
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
                let name = g.pick(&[
                    "in_t.dat",
                    "in_t.dat",
                    "in_t.stats",
                    "in_tbl",
                    "in_t.DAT",
                    "in_t.",
                ]);
                format!("lst {name} {}", hex(&fs_table(&mut g)))
            }
            2 => {
                let name = g.pick(&["in_t.csv", "in_t.CSV", "in_t.csv"]);
                let text = if g.chance(4) {
                    fs_table(&mut g)
                } else {
                    csv_text(&mut g)
                };
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
            4 => format!(
                "lst {} -",
                g.pick(&["in_none.dat", "in_none.csv", "in_none.tsv", "in_none"])
            ),
            5 | 6 => {
                let w = g.int(1, 4) as i32;
                let nf = g.int(1, 3) as i32;
                let vol = vol_tokens(&mut g, w, nf);
                let (t, h) = template(&mut g, w, nf);
                if k % 20 == 5 {
                    format!("ist {vol} {t} {h}")
                } else {
                    format!(
                        "wst {vol} {t} {h} {}",
                        g.pick(&["out.txt", "out.txt", "nodir/out.txt"])
                    )
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
    dia(
        &mut d,
        "reorder_vals",
        o.reorder_vals.as_ref().map(|v| &v[..]),
        3,
    );
    di(&mut d, "reorder4_flag", o.reorder4_flag.into());
    dia(
        &mut d,
        "reorder4_vals",
        o.reorder4_vals.as_ref().map(|v| &v[..]),
        4,
    );
    di(&mut d, "shiftdim_flag", o.shiftdim_flag.into());
    dia(&mut d, "shift_dim", o.shift_dim.as_ref().map(|v| &v[..]), 3);
    df(&mut d, "invert_val", fl(o.invert_val));
    ds(&mut d, "in_name", s(&o.in_name));
    ds(&mut d, "out_name", s(&o.out_name));
    di(&mut d, "conform_flag", o.conform_flag.into());
    di(&mut d, "ConfKeepDC", o.conf_keep_dc.into());
    di(
        &mut d,
        "conform_width_256_flag",
        o.conform_width_256_flag.into(),
    );
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
    di(
        &mut d,
        "invert_transform_flag",
        o.invert_transform_flag.into(),
    );
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
        di(
            &mut d,
            &format!("in_{a}_size_flag"),
            o.in_size_flag[k].into(),
        );
        df(&mut d, &format!("in_{a}_size"), o.in_size[k]);
    }
    for (k, a) in ["i", "j", "k"].iter().enumerate() {
        di(
            &mut d,
            &format!("out_{a}_size_flag"),
            o.out_size_flag[k].into(),
        );
        df(&mut d, &format!("out_{a}_size"), o.out_size[k]);
    }
    for (k, a) in ["i", "j", "k"].iter().enumerate() {
        di(
            &mut d,
            &format!("in_{a}_direction_flag"),
            o.in_direction_flag[k].into(),
        );
        dfa(&mut d, &format!("in_{a}_directions"), o.in_directions[k]);
    }
    for (k, a) in ["i", "j", "k"].iter().enumerate() {
        di(
            &mut d,
            &format!("out_{a}_direction_flag"),
            o.out_direction_flag[k].into(),
        );
        dfa(&mut d, &format!("out_{a}_directions"), o.out_directions[k]);
    }
    ds(&mut d, "colortablefile", s(&o.colortablefile));
    di(&mut d, "in_orientation_flag", o.in_orientation_flag.into());
    ds(
        &mut d,
        "in_orientation_string",
        o.in_orientation_string.as_deref(),
    );
    di(
        &mut d,
        "out_orientation_flag",
        o.out_orientation_flag.into(),
    );
    ds(
        &mut d,
        "out_orientation_string",
        o.out_orientation_string.as_deref(),
    );
    di(&mut d, "in_center_flag", o.in_center_flag.into());
    dfa(&mut d, "in_center", o.in_center);
    di(
        &mut d,
        "delta_in_center_flag",
        o.delta_in_center_flag.into(),
    );
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
    ds(
        &mut d,
        "out_data_type_string",
        o.out_data_type_string.as_deref(),
    );
    df(&mut d, "rescale_factor", fl(o.rescale_factor));
    df(&mut d, "scale_factor", fl(o.scale_factor));
    df(&mut d, "out_scale_factor", fl(o.out_scale_factor));
    di(&mut d, "rescale_voxel", o.rescale_voxel.into());
    for (k, a) in ["c", "r", "s"].iter().enumerate() {
        diu(
            &mut d,
            &format!("rescale_voxel_{a}"),
            o.rescale_voxel_crs[k],
        );
    }
    ds(&mut d, "resample_type", o.resample_type.as_deref());
    di(&mut d, "resample_type_val", o.resample_type_val.into());
    di(&mut d, "force_in_type_flag", o.force_in_type_flag.into());
    di(&mut d, "forced_in_type", o.forced_in_type.into());
    ds(&mut d, "in_type_string", s(&o.in_type_string));
    di(&mut d, "force_out_type_flag", o.force_out_type_flag.into());
    di(&mut d, "forced_out_type", o.forced_out_type.into());
    ds(&mut d, "out_type_string", o.out_type_string.as_deref());
    di(
        &mut d,
        "force_template_type_flag",
        o.force_template_type_flag.into(),
    );
    di(
        &mut d,
        "forced_template_type",
        o.forced_template_type.into(),
    );
    ds(
        &mut d,
        "template_type_string",
        o.template_type_string.as_deref(),
    );
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
    for (k, a) in ["SubSampStart", "SubSampDelta", "SubSampEnd"]
        .iter()
        .enumerate()
    {
        diu(&mut d, a, o.sub_samp[k]);
    }
    di(&mut d, "in_like_flag", o.in_like_flag.into());
    ds(&mut d, "in_like_name", o.in_like_name.as_deref());
    di(&mut d, "FirstDicom", o.first_dicom.into());
    ds(&mut d, "FirstDicomFile", o.first_dicom_file.as_deref());
    di(&mut d, "roi_flag", o.roi_flag.into());
    di(
        &mut d,
        "fill_parcellation_flag",
        o.fill_parcellation_flag.into(),
    );
    di(&mut d, "zero_outlines_flag", o.zero_outlines_flag.into());
    di(
        &mut d,
        "smooth_parcellation_flag",
        o.smooth_parcellation_flag.into(),
    );
    diu(
        &mut d,
        "smooth_parcellation_count",
        o.smooth_parcellation_count,
    );
    di(&mut d, "color_file_flag", o.color_file_flag.into());
    ds(&mut d, "color_file_name", o.color_file_name.as_deref());
    di(
        &mut d,
        "translate_labels_flag",
        o.translate_labels_flag.into(),
    );
    di(&mut d, "no_scale_flag", o.no_scale_flag.into());
    di(&mut d, "nthframe", o.nthframe.into());
    di(&mut d, "LeftRightReverse", o.left_right_reverse.into());
    di(
        &mut d,
        "LeftRightReversePix",
        o.left_right_reverse_pix.into(),
    );
    di(
        &mut d,
        "LeftRightMirrorFlag",
        o.left_right_mirror_flag.into(),
    );
    di(&mut d, "LeftRightKeepFlag", o.left_right_keep_flag.into());
    ds(
        &mut d,
        "LeftRightMirrorHemi",
        o.left_right_mirror_hemi.as_deref(),
    );
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
    di(
        &mut d,
        "DCM2NIIX_no_ForceStackSameSeries",
        x.dcm2niix_no_force_stack_same_series.into(),
    );
    dp(
        &mut d,
        "DCM2NIIX_INFO_DUMP",
        x.dcm2niix_info_dump.as_deref(),
    );
    dp(
        &mut d,
        "DCM2NIIX_DICOM_FLIST",
        x.dcm2niix_dicom_flist.as_deref(),
    );
    di(
        &mut d,
        "DCM2NIIX_OnlySingleFile",
        x.dcm2niix_only_single_file.into(),
    );
    dp(&mut d, "DCM2NIIX_OPTS", x.dcm2niix_opts.as_deref());
    di(&mut d, "DoSiemensAsciiDump", x.do_siemens_ascii_dump.into());
    di(
        &mut d,
        "DoSiemensAsciiAltDump",
        x.do_siemens_ascii_alt_dump.into(),
    );
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
        x.first_dicom_file
            .first()
            .map_or(0, |&c| i64::from(c as i8)),
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
                        res += &oh(Some(
                            &f[..f.iter().position(|&c| c == 0).unwrap_or(f.len())],
                        ));
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
            "cnv" => {
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
                // the official child's standard error: a file (write-only), written to only by
                // the case
                let mut tb = larmorx_freesurfer_core::log::TrackedErr::new(&g, &mut b);
                let mut r = Run {
                    out: &mut a,
                    err: &mut tb,
                    env: &mut env,
                    dir: Some(dir),
                };
                // main, with fsconvert's canonical command line (canon_info)
                let mut o = parse(&mut g, &mut r, &argv)?;
                larmorx_freesurfer_volumes::mri_convert::options::check_gdf_stem(&g, &mut r, &o)?;
                o.cmdline = canon_info(&o.cmdline);
                larmorx_freesurfer_volumes::mri_convert::main::convert(&mut g, &mut r, o)?;
                res += "RETURNED ";
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
    let files = if cmd == "cnv" {
        written_full(dir)
    } else {
        written(dir)
    };
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

// ----- whole conversions (`cnv`, relay 47) -----

/// A small volume of a conversion case.
struct Vol {
    dims: [i32; 4],
    ty: i32,
    size: [f32; 3],
    /// `x_r x_a x_s y_r y_a y_s z_r z_a z_s`.
    mdc: [f32; 9],
    c: [f32; 3],
    good: i16,
    vox: Vec<f32>,
    /// TR, flip angle, TE, TI, FoV after the voxels.
    scan: Option<[f32; 5]>,
}

/// The MGH type codes the cases write: uchar, int, float, short.
const MGH_TYPES: [i32; 4] = [MRI_UCHAR, MRI_INT, MRI_FLOAT, MRI_SHORT];

/// Direction cosines: a signed permutation (most), an oblique rotation by (0.6, 0.8), or a
/// degenerate set (two axes alike).
fn rand_mdc(g: &mut Gen) -> [f32; 9] {
    let mut m = [0f32; 9];
    match g.below(10) {
        0 => {
            let (a, b) = (0.6f32, 0.8f32);
            let cols = match g.below(3) {
                0 => [[a, b, 0.0], [-b, a, 0.0], [0.0, 0.0, 1.0]],
                1 => [[1.0, 0.0, 0.0], [0.0, a, b], [0.0, -b, a]],
                _ => [[a, 0.0, -b], [0.0, 1.0, 0.0], [b, 0.0, a]],
            };
            for k in 0..3 {
                m[3 * k..3 * k + 3].copy_from_slice(&cols[k]);
            }
        }
        1 => {
            // two axes alike (not orthogonal), or no axis left-right
            let cols = if g.chance(2) {
                [[-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
            } else {
                [[0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [0.0, 1.0, 0.0]]
            };
            for k in 0..3 {
                m[3 * k..3 * k + 3].copy_from_slice(&cols[k]);
            }
        }
        _ => {
            let perms = [
                [0, 1, 2],
                [0, 2, 1],
                [1, 0, 2],
                [1, 2, 0],
                [2, 0, 1],
                [2, 1, 0],
            ];
            let p = g.pick(&perms);
            for k in 0..3 {
                let s = if g.chance(2) { -1.0 } else { 1.0 };
                m[3 * k + p[k]] = s;
            }
        }
    }
    m
}

fn rand_vol(g: &mut Gen, dims: [i32; 4], ty: i32) -> Vol {
    let sizes = [1.0f32, 1.0, 0.5, 2.0, 1.5, 3.0];
    let size = [g.pick(&sizes), g.pick(&sizes), g.pick(&sizes)];
    let c = [
        g.int(-8, 8) as f32 * 0.5,
        g.int(-8, 8) as f32 * 0.25,
        g.int(-8, 8) as f32,
    ];
    let n = dims.iter().product::<i32>() as usize;
    let labels = [0.0f32, 0.0, 2.0, 3.0, 41.0, 42.0, 17.0, 53.0, 4.0, 43.0];
    let seg = g.chance(4);
    let mut vox = Vec::with_capacity(n);
    for _ in 0..n {
        let v = if seg {
            g.pick(&labels)
        } else {
            match ty {
                MRI_UCHAR => g.int(0, 12) as f32 * if g.chance(3) { 20.0 } else { 1.0 },
                MRI_FLOAT => {
                    if g.chance(60) {
                        f32::NAN
                    } else {
                        g.int(-40, 200) as f32 * 0.125
                    }
                }
                _ => g.int(-5, 60) as f32,
            }
        };
        vox.push(if ty == MRI_UCHAR {
            v.clamp(0.0, 255.0)
        } else {
            v
        });
    }
    Vol {
        dims,
        ty,
        size,
        mdc: rand_mdc(g),
        c,
        good: if g.chance(8) { 0 } else { 1 },
        vox,
        scan: if g.chance(2) {
            Some([2000.0, std::f32::consts::FRAC_PI_6, 30.0, 900.0, 256.0])
        } else {
            None
        },
    }
}

/// The bytes of an `.mgh` file of `v` (big-endian: the header padded to 284 bytes, the voxels
/// column fastest and frame slowest, then the scan parameters when given).
fn mgh_bytes(v: &Vol) -> Vec<u8> {
    let mut b: Vec<u8> = Vec::new();
    for x in [1, v.dims[0], v.dims[1], v.dims[2], v.dims[3], v.ty, 0] {
        b.extend_from_slice(&x.to_be_bytes());
    }
    b.extend_from_slice(&v.good.to_be_bytes());
    for x in v.size.iter().chain(v.mdc.iter()).chain(v.c.iter()) {
        b.extend_from_slice(&x.to_be_bytes());
    }
    b.resize(284, 0);
    for &x in &v.vox {
        match v.ty {
            MRI_UCHAR => b.push(x as u8),
            MRI_INT => b.extend_from_slice(&(x as i32).to_be_bytes()),
            MRI_SHORT => b.extend_from_slice(&(x as i16).to_be_bytes()),
            _ => b.extend_from_slice(&x.to_be_bytes()),
        }
    }
    if let Some(s) = v.scan {
        for x in s {
            b.extend_from_slice(&x.to_be_bytes());
        }
    }
    b
}

/// A `cnv` case of the given files and arguments.
fn cnv_line(files: &[(String, Vec<u8>)], args: &[String]) -> String {
    let mut t = vec!["cnv".to_owned(), files.len().to_string()];
    for (f, c) in files {
        t.push(f.clone());
        t.push(hex(c));
    }
    t.push(args.len().to_string());
    t.extend(args.iter().map(|a| hex(a.as_bytes())));
    t.join(" ")
}

/// An LTA file's text: one transform of `ltype` (0 voxel-to-voxel, 1 RAS-to-RAS), the matrix
/// rows, the source and destination geometries (`None`: not valid).
fn lta_text(ltype: i32, m: [[f32; 4]; 4], src: Option<&Vol>, dst: Option<&Vol>) -> Vec<u8> {
    let mut s = format!(
        "# transform file in_t.lta\ntype      = {ltype}\nnxforms   = 1\nmean      = 0.0000 0.0000 0.0000\nsigma     = 1.0000\n1 4 4\n"
    );
    for row in m {
        s += &format!("{} {} {} {}\n", row[0], row[1], row[2], row[3]);
    }
    for (name, v) in [("src", src), ("dst", dst)] {
        s += &format!("{name} volume info\n");
        match v {
            None => s += "valid = 0  # volume info invalid\n",
            Some(v) => {
                s += "valid = 1  # volume info valid\nfilename = in_v.mgh\n";
                s += &format!("volume = {} {} {}\n", v.dims[0], v.dims[1], v.dims[2]);
                s += &format!("voxelsize = {} {} {}\n", v.size[0], v.size[1], v.size[2]);
                for (k, a) in ["xras", "yras", "zras"].iter().enumerate() {
                    s += &format!(
                        "{a}   = {} {} {}\n",
                        v.mdc[3 * k],
                        v.mdc[3 * k + 1],
                        v.mdc[3 * k + 2]
                    );
                }
                s += &format!("cras   = {} {} {}\n", v.c[0], v.c[1], v.c[2]);
            }
        }
    }
    s.into_bytes()
}

const LUT: &[u8] = b"0 Unknown 0 0 0 0\n2 Left-Cerebral-White-Matter 245 245 245 0\n3 Left-Cerebral-Cortex 205 62 78 0\n41 Right-Cerebral-White-Matter 0 225 0 0\n42 Right-Cerebral-Cortex 205 62 78 0\n";

/// One whole conversion: a small MGH input, up to three options with arguments they accept (a
/// few combinations conflict), the output in one of five formats.
fn cnv_case(g: &mut Gen) -> String {
    let dims = [
        g.int(1, 6) as i32,
        g.int(1, 6) as i32,
        if g.chance(6) { 1 } else { g.int(2, 6) as i32 },
        if g.chance(3) { g.int(2, 4) as i32 } else { 1 },
    ];
    let ty = g.pick(&MGH_TYPES);
    let v = rand_vol(g, dims, ty);
    let mut files: Vec<(String, Vec<u8>)> = vec![("in_v.mgh".into(), mgh_bytes(&v))];
    let mut args: Vec<String> = Vec::new();
    let a = |xs: &[&str]| -> Vec<String> { xs.iter().map(|s| (*s).to_owned()).collect() };
    let f3 = |g: &mut Gen, xs: &[&str]| -> Vec<String> {
        (0..3).map(|_| g.pick(xs).to_owned()).collect()
    };
    let nopt = g.int(1, 3);
    for _ in 0..nopt {
        let mut o: Vec<String> = match g.below(61) {
            0 => a(&["-odt", g.pick(&["uchar", "short", "int", "float"])]),
            1 => a(&["-odt", g.pick(&["uchar", "short"]), "-ns", "1"]),
            2 => a(&["-nc", "-odt", "uchar"]),
            3 => a(&[
                "-rt",
                g.pick(&["nearest", "interpolate", "weighted", "vote", "cubic"]),
            ]),
            4 => {
                let mut x = a(&["-vs"]);
                x.extend(f3(g, &["1", "2", "0.75", "1.5", "3"]));
                x
            }
            5 => {
                let mut x = a(&[g.pick(&["-ds", "-dsold"])]);
                x.extend(f3(g, &["1", "2", "1.5", "3"]));
                x
            }
            6 => a(&[
                g.pick(&["-ois", "-ojs", "-oks"]),
                g.pick(&["0.5", "2", "1.5"]),
            ]),
            7 => a(&[
                g.pick(&["-oni", "-onj", "-onk"]),
                g.pick(&["1", "3", "5", "8"]),
            ]),
            8 => a(&[
                g.pick(&["-iis", "-ijs", "-iks"]),
                g.pick(&["0.5", "2", "1.25"]),
            ]),
            9 => {
                let mut x = a(&[g.pick(&["-iid", "-ijd", "-ikd", "-oid", "-ojd", "-okd"])]);
                x.extend(
                    g.pick(&[
                        ["1", "0", "0"],
                        ["0", "-1", "0"],
                        ["0", "0", "1"],
                        ["0.6", "0.8", "0"],
                        ["1", "1", "0"],
                    ])
                    .iter()
                    .map(|s| (*s).to_owned()),
                );
                x
            }
            10 => a(&[
                g.pick(&["-io", "-oo"]),
                g.pick(&["LIA", "RAS", "LPS", "PIL", "SAR", "RSA"]),
            ]),
            11 => {
                let mut x = a(&[g.pick(&["-ic", "-dic", "-oc"])]);
                x.extend(f3(g, &["0", "1.5", "-2", "10"]));
                x
            }
            12 => a(&[
                g.pick(&["-tr", "-te", "-TI", "-flip_angle"]),
                g.pick(&["2.5", "0.7", "1000"]),
            ]),
            13 => a(&[
                g.pick(&["-sc", "-osc", "--rescale"]),
                g.pick(&["2", "0.5", "100", "-1"]),
            ]),
            14 => a(&["--rescale-voxel", "0", "0", g.pick(&["0", "1"])]),
            15 => a(&["-nth", g.pick(&["0", "1", "5"])]),
            16 => {
                let mut x = a(&["-f"]);
                for _ in 0..g.int(1, 3) {
                    x.push(g.int(0, 3).to_string());
                }
                x
            }
            17 => a(&["--mid-frame"]),
            18 => a(&[g.pick(&["--nskip", "--ndrop"]), g.pick(&["1", "2"])]),
            19 => a(&[
                "--fsubsample",
                "0",
                g.pick(&["1", "2"]),
                g.pick(&["1", "3"]),
            ]),
            20 => a(&["--cutends", g.pick(&["0", "1", "2"])]),
            21 => {
                let mut x = a(&["--crop"]);
                x.extend(f3(g, &["1", "2", "3"]));
                if g.chance(2) {
                    x.push("--cropsize".into());
                    x.extend(f3(g, &["1", "2", "3", "4"]));
                }
                x
            }
            22 => a(&[
                "--slice-crop",
                g.pick(&["0", "1"]),
                g.pick(&["1", "2", "3"]),
            ]),
            23 => a(&["--slice-reverse"]),
            24 => a(&["--slice-bias", g.pick(&["0.5", "1", "1.5"])]),
            25 => a(&["--reduce", "1"]),
            26 => a(&["--fwhm", g.pick(&["1", "2.5"])]),
            27 => {
                let mut x = a(&["--antialias", "-vs"]);
                x.extend(f3(g, &["1", "2", "3"]));
                x
            }
            28 => a(&["-ut", g.pick(&["3", "10.5"])]),
            29 => a(&["-oval", g.pick(&["0", "7"])]),
            30 => a(&["--upsample", "2"]),
            31 => a(&["--left-right-reverse"]),
            32 => a(&["--left-right-reverse-pix"]),
            33 => a(&[
                g.pick(&["--left-right-mirror", "--left-right-keep"]),
                g.pick(&["lh", "rh"]),
            ]),
            34 => a(&["--left-right-swap-label"]),
            35 => a(&["--flip-cols"]),
            36 => a(&["--invert_contrast", g.pick(&["0", "5"])]),
            37 => a(&[
                "--shift",
                g.pick(&["1", "2", "3"]),
                g.pick(&["1", "-1", "2"]),
                g.pick(&["0", "1"]),
            ]),
            38 => a(&[
                "-r",
                g.pick(&["1", "-1", "2"]),
                g.pick(&["2", "3", "-3"]),
                g.pick(&["3", "1", "-2"]),
            ]),
            39 => a(&["-r4", "1", "2", "3", "4"]),
            40 => a(&[g.pick(&["-ii", "-oi", "-ti", "-is", "-os", "-im", "-om"])]),
            41 => a(&[g.pick(&["-ro", "-nw"])]),
            42 => a(&["--split"]),
            43 => a(&[g.pick(&["--ascii", "--ascii+crsf", "--ascii-fcol"])]),
            44 => a(&["--delete-cmds"]),
            45 => a(&["--new-transform-fname", "talairach.xfm"]),
            46 => a(&["-sn", "bert"]),
            47 => a(&["--sphinx"]),
            48 => a(&[g.pick(&["--erode-seg", "--dil-seg"]), g.pick(&["1", "2"])]),
            49 => {
                files.push(("in_lut.txt".into(), LUT.to_vec()));
                a(&["--ctab", g.pick(&["in_lut.txt", "remove"])])
            }
            50 | 51 => {
                // another volume: the template, --like or --in_like
                let same = g.chance(2);
                let d2 = if same {
                    dims
                } else {
                    [
                        g.int(1, 6) as i32,
                        g.int(1, 6) as i32,
                        g.int(1, 6) as i32,
                        1,
                    ]
                };
                let t2 = g.pick(&MGH_TYPES);
                let v2 = rand_vol(g, d2, t2);
                files.push(("in_l.mgh".into(), mgh_bytes(&v2)));
                a(&[g.pick(&["-rl", "--like", "-il"]), "in_l.mgh"])
            }
            52..=54 => {
                let ltype = if g.chance(2) { 1 } else { 0 };
                let m = match g.below(3) {
                    0 => [
                        [1.0, 0.0, 0.0, 0.0],
                        [0.0, 1.0, 0.0, 0.0],
                        [0.0, 0.0, 1.0, 0.0],
                        [0.0, 0.0, 0.0, 1.0],
                    ],
                    1 => [
                        [1.0, 0.0, 0.0, 1.0],
                        [0.0, 1.0, 0.0, -2.0],
                        [0.0, 0.0, 1.0, 0.5],
                        [0.0, 0.0, 0.0, 1.0],
                    ],
                    _ => [
                        [0.0, 1.0, 0.0, 0.0],
                        [-1.0, 0.0, 0.0, 3.0],
                        [0.0, 0.0, 2.0, 0.0],
                        [0.0, 0.0, 0.0, 1.0],
                    ],
                };
                let src = if g.chance(3) { None } else { Some(&v) };
                let dst = if g.chance(2) { None } else { Some(&v) };
                files.push(("in_t.lta".into(), lta_text(ltype, m, src, dst)));
                a(&[g.pick(&["-at", "-ait"]), "in_t.lta"])
            }
            55 => a(&["--store_orig_ras2vox"]),
            56 => {
                files.push((
                    "in_m0.txt".into(),
                    b"1 0 0 4\n0 1 0 -2\n0 0 1 3.5\n0 0 0 1\n".to_vec(),
                ));
                a(&["--autoalign", "in_m0.txt"])
            }
            57 => a(&["--force_ras_good"]),
            58 => a(&["-zgez"]),
            59 => a(&["--diag", "1", "-at", "in_t.lta"]),
            // the conformed outputs (at least 256 voxels per axis) are generate_cnv_heavy's
            _ => a(&["--no_scale", "1"]),
        };
        if o.first().is_some_and(|x| x == "--diag") {
            // the transform the LTA dump prints
            files.push((
                "in_t.lta".into(),
                lta_text(
                    1,
                    [
                        [1.0, 0.0, 0.0, 1.0],
                        [0.0, 1.0, 0.0, 0.0],
                        [0.0, 0.0, 1.0, 0.0],
                        [0.0, 0.0, 0.0, 1.0],
                    ],
                    Some(&v),
                    Some(&v),
                ),
            ));
        }
        args.append(&mut o);
    }
    let out = g.pick(&[
        "o.mgh", "o.mgh", "o.mgh", "o.mgz", "o.nii", "o.nii.gz", "o.img",
    ]);
    args.push("in_v.mgh".into());
    args.push(out.into());
    // a file given twice keeps its last contents (both sides write them in order)
    cnv_line(&files, &args)
}

/// The conversions every seed starts with: each option's path once on a 4 x 3 x 2 float volume
/// of two frames.
fn cnv_prelude() -> Vec<String> {
    let mut g = Gen::new(12345);
    let v = rand_vol(&mut g, [4, 3, 2, 2], MRI_FLOAT);
    let files = vec![("in_v.mgh".to_owned(), mgh_bytes(&v))];
    let lines: Vec<Vec<&str>> = vec![
        vec![],
        vec!["-odt", "uchar"],
        vec!["-odt", "short", "-ns", "1"],
        vec!["-vs", "2", "2", "2"],
        vec!["-ds", "2", "2", "1"],
        vec!["-dsold", "2", "2", "1"],
        vec!["--antialias", "-vs", "2", "2", "2"],
        vec!["--left-right-reverse"],
        vec!["--left-right-reverse-pix"],
        vec!["--left-right-mirror", "lh"],
        vec!["--left-right-keep", "rh"],
        vec!["--flip-cols"],
        vec!["--slice-reverse"],
        vec!["--slice-bias", "0.5"],
        vec!["--fwhm", "2"],
        vec!["--rescale", "10"],
        vec!["-sc", "2", "-osc", "0.5"],
        vec!["-ii", "-oi", "-ti", "-is", "-os", "-im", "-om"],
        vec!["-ro", "-ii"],
        vec!["-nw"],
        vec!["--split"],
        vec!["--ascii"],
        vec!["--ascii+crsf"],
        vec!["--ascii-fcol"],
        vec!["-f", "1", "0"],
        vec!["--mid-frame"],
        vec!["--ndrop", "1"],
        vec!["--nskip", "1"],
        vec!["--crop", "2", "1", "1", "--cropsize", "2", "2", "1"],
        vec!["--rescale-voxel", "1", "1", "0"],
        vec!["--sphinx"],
        vec!["--invert_contrast", "1"],
        vec!["-r", "2", "1", "3"],
        vec!["--shift", "1", "1", "1"],
        vec!["-oni", "6", "-ois", "0.5"],
        vec!["-iid", "1", "1", "0"],
        vec!["--upsample", "2"],
        vec!["--reduce", "1"],
        vec!["-ut", "5"],
        vec!["--store_orig_ras2vox"],
        vec![
            "--delete-cmds",
            "--new-transform-fname",
            "x.xfm",
            "-sn",
            "bert",
        ],
        // a register.dat and a linear transform that are not there (the C library's perror)
        vec!["-at", "in_missing.dat"],
        vec!["-at", "in_missing.lta"],
        vec!["-at", "in_missing.xfm"],
        vec!["-ait", "in_missing.dat"],
        vec![
            "--left-right-reverse",
            "-iid",
            "0",
            "1",
            "0",
            "-ijd",
            "0",
            "0",
            "1",
            "-ikd",
            "0",
            "1",
            "0",
        ],
        vec!["--like", "in_missing.mgh"],
        vec!["--ascii", "-o", "nodir/o.txt"],
    ];
    let mut out = Vec::new();
    for l in lines {
        let mut args: Vec<String> = l.iter().map(|s| (*s).to_owned()).collect();
        args.push("in_v.mgh".into());
        if !l.contains(&"-o") {
            args.push("o.mgh".into());
        }
        out.push(cnv_line(&files, &args));
    }
    out
}

/// The conformed conversions (outputs of 256 x 256 x 256 voxels and more: too slow for the
/// recorded test's debug build): each conform option on small inputs of each type.
pub fn generate_cnv_heavy(seed: u64) -> Vec<String> {
    let mut g = Gen::new(seed ^ 0x6865_6176);
    let opts: [&[&str]; 8] = [
        &["-c"],
        &["--conform"],
        &["-c", "-cs", "2"],
        &["-cm"],
        &["--conform-dc"],
        &["-c", "--cw256", "-cs", "4"],
        &["-c", "-odt", "float", "-rt", "nearest"],
        &["-c", "-oc", "1", "2", "3"],
    ];
    let mut v = Vec::new();
    for (k, o) in opts.iter().enumerate() {
        let ty = MGH_TYPES[k % 4];
        let dims = [
            g.int(2, 5) as i32,
            g.int(2, 5) as i32,
            g.int(2, 5) as i32,
            1,
        ];
        let vol = rand_vol(&mut g, dims, ty);
        let files = vec![("in_v.mgh".to_owned(), mgh_bytes(&vol))];
        let mut args: Vec<String> = o.iter().map(|s| (*s).to_owned()).collect();
        args.push("in_v.mgh".into());
        args.push(g.pick(&["o.mgh", "o.mgz"]).into());
        v.push(cnv_line(&files, &args));
    }
    v
}

/// `n` conversion cases of seed `seed`: the [`cnv_prelude`], then seeded ones.
pub fn generate_cnv(seed: u64, n: usize) -> Vec<String> {
    let mut v = cnv_prelude();
    v.truncate(n);
    let mut g = Gen::new(seed ^ 0x636e_7600);
    while v.len() < n {
        v.push(cnv_case(&mut g));
    }
    v
}

/// `fsconvert`'s `canon_info`: `getAllInfo`'s text with the time, the user, the machine and its
/// kernel's version as `X`.
fn canon_info(s: &str) -> String {
    let mut r = s.to_owned();
    let mut blank = |key: &str, next: &str| {
        let Some(a) = r.find(key) else {
            return;
        };
        let a = a + key.len();
        let Some(b) = r[a..].find(next) else {
            return;
        };
        r.replace_range(a..a + b, "X");
    };
    blank("TimeStamp: ", "  BuildTime: ");
    blank("User: ", "  Machine: ");
    blank("Machine: ", "  Platform: ");
    blank("PlatformVersion: ", "  CompilerName: ");
    r
}

/// `fsconvert`'s `slurp_gz_masked`: a file's bytes through zlib's `gzread` (a gzip file
/// decompressed), each command line's text from `TimeStamp: ` to its NUL masked with `X`.
fn slurp_gz_masked(path: &Path) -> Vec<u8> {
    let mut r = Vec::new();
    let name = path.to_string_lossy().into_owned();
    let Some(mut f) = larmorx_io::znz::GzFile::open(name.as_bytes(), b"rb") else {
        return r;
    };
    let mut b = vec![0u8; 65536];
    loop {
        let n = f.gzread(&mut b);
        if n <= 0 {
            break;
        }
        r.extend_from_slice(&b[..n as usize]);
    }
    let key = b"TimeStamp: ";
    let mut at = 0;
    while let Some(p) = r[at..].windows(key.len()).position(|w| w == key) {
        let mut k = at + p + key.len();
        while k < r.len() && r[k] != 0 {
            r[k] = b'X';
            k += 1;
        }
        at = at + p + 1;
    }
    r
}

/// The files a `cnv` case wrote (names not starting with `in_`), sorted: `F <name> <bytes>
/// <FNV-1a>` of [`slurp_gz_masked`].
fn written_full(dir: &Path) -> String {
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
        let b = slurp_gz_masked(&dir.join(&n));
        s += &format!("F {n} {} {:016x} ", b.len(), fnv(&b));
    }
    s.trim_end().to_owned()
}

// ----- outline (.otl) series (`otl`, relay 48) -----

/// The outline cases' colour table: FreeSurfer's names of a few structures (an exterior one for
/// the translation), a name in lower case and one without its hemisphere (the second names a
/// label is compared with), entry 5307 (FS-1055) and one past it (refused by the claims).
const OTL_LUT: &[u8] = b"0 Unknown 0 0 0 0\n1 Left-Cerebral-Exterior 70 130 180 0\n2 Left-Cerebral-White-Matter 245 245 245 0\n3 Left-Cerebral-Cortex 205 62 78 0\n4 Left-Lateral-Ventricle 120 18 134 0\n17 Left-Hippocampus 220 216 20 0\n24 CSF 60 60 60 0\n41 Right-Cerebral-White-Matter 0 225 0 0\n42 Right-Cerebral-Cortex 205 62 78 0\n53 Right-Hippocampus 220 216 20 0\n98 csf 1 2 3 0\n120 Hippocampus 5 6 7 0\n5307 Test-Unc 1 2 3 0\n6000 Beyond-Max 4 5 6 0\n";

/// The labels an outline names: the table's, written as CMA outlines write them (spaces for
/// dashes, a hemisphere prefix, leading and trailing blanks, another case), exterior labels,
/// unknown ones and the two past the claims' range.
const OTL_LABELS: &[&str] = &[
    "Left-Cerebral-Cortex",
    "Left Cerebral Cortex",
    "Right-Cerebral-White-Matter",
    "Right Cerebral Cortex",
    "Left-Cerebral-Exterior",
    "Right Cerebral Exterior",
    "Left-Hippocampus",
    "Right-Hippocampus",
    "left-Hippocampus",
    "  Left-Lateral-Ventricle  ",
    "Left-Lateral-Ventricle--",
    "CSF",
    "csf",
    "Unknown",
    "Test-Unc",
    "Test Unc",
    "Beyond-Max",
    "Mystery",
    "Another Mystery",
    "---",
    "Exterior and cortex, Cortex and exterior",
];

/// One outline of an `.otl` file: its label line's text, seed, points, type (`None`: the main
/// header's) and the header lines added (another `COL_NUM`, a repeated line).
struct Outline {
    label: Option<String>,
    seed: (i32, i32),
    points: Vec<(i32, i32)>,
    otype: Option<&'static str>,
    extra: String,
}

/// The rectangle from (`x0`, `y0`) to (`x1`, `y1`): its boundary, 4-connected.
fn otl_rect(x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<(i32, i32)> {
    let mut p = Vec::new();
    for x in x0..=x1 {
        p.push((x, y0));
        p.push((x, y1));
    }
    for y in y0 + 1..y1 {
        p.push((x0, y));
        p.push((x1, y));
    }
    p
}

/// A diamond around (`cx`, `cy`): a boundary of diagonal steps, which a 4-connected fill crosses.
fn otl_diamond(cx: i32, cy: i32, r: i32) -> Vec<(i32, i32)> {
    let mut p = Vec::new();
    for k in 0..r {
        p.push((cx + k, cy - r + k));
        p.push((cx + r - k, cy + k));
        p.push((cx - k, cy + r - k));
        p.push((cx - r + k, cy - k));
    }
    p
}

/// An `.otl` file's bytes: the version line, the main header (`ONUM` the outlines' count unless
/// `onum` says otherwise, `COL_NUM 2`, the global `TYPE` when given), each outline's header and
/// points (ascii lines, or big-endian shorts for `short`).
fn otl_bytes(outlines: &[Outline], global_type: Option<&str>, onum: Option<&str>) -> Vec<u8> {
    let mut s = b"GDF FILE VERSION 1\nSTART MAIN HEADER\n".to_vec();
    match onum {
        Some(o) => s.extend_from_slice(o.as_bytes()),
        None => s.extend_from_slice(format!("ONUM {}\n", outlines.len()).as_bytes()),
    }
    s.extend_from_slice(b"COL_NUM 2\n");
    if let Some(t) = global_type {
        s.extend_from_slice(format!("TYPE {t}\n").as_bytes());
    }
    s.extend_from_slice(b"END MAIN HEADER\n");
    for o in outlines {
        s.extend_from_slice(b"START GDF HEADER\n");
        s.extend_from_slice(format!("ROW_NUM {}\n", o.points.len()).as_bytes());
        s.extend_from_slice(format!("SEED {} {}\n", o.seed.0, o.seed.1).as_bytes());
        if let Some(l) = &o.label {
            s.extend_from_slice(format!("LABEL {l}\n").as_bytes());
        }
        if let Some(t) = o.otype {
            s.extend_from_slice(format!("TYPE {t}\n").as_bytes());
        }
        s.extend_from_slice(o.extra.as_bytes());
        s.extend_from_slice(b"END GDF HEADER\nSTART POINTS\n");
        let binary = o.otype.or(global_type) == Some("short");
        for &(x, y) in &o.points {
            if binary {
                s.extend_from_slice(&(x as i16).to_be_bytes());
                s.extend_from_slice(&(y as i16).to_be_bytes());
            } else {
                s.extend_from_slice(format!("{x} {y}\n").as_bytes());
            }
        }
        s.extend_from_slice(b"END POINTS\n");
    }
    s
}

/// A header-only `.mgh` file (no voxels) of `w` x `h` x `d`: `--in_like`'s size for the series.
fn otl_like(w: i32, h: i32, d: i32) -> Vec<u8> {
    let mut b = Vec::new();
    for x in [1, w, h, d, 1, 0, 0] {
        b.extend_from_slice(&x.to_be_bytes());
    }
    b.extend_from_slice(&0i16.to_be_bytes());
    b.resize(284, 0);
    b
}

/// An outline inside the outline field of `fw` x `fh` places (at most 512 each): a closed
/// rectangle with its seed inside (most), the seed outside or on the outline, a diamond (the fill
/// leaks through it), scattered points, or none.
fn otl_outline(g: &mut Gen, fw: i32, fh: i32, leak: bool) -> Outline {
    let (fw, fh) = (fw.min(512), fh.min(512));
    let x0 = g.int(0, i64::from(fw - 12)) as i32;
    let y0 = g.int(0, i64::from(fh - 12)) as i32;
    let x1 = (x0 + g.int(2, 40) as i32).min(fw - 1);
    let y1 = (y0 + g.int(2, 40) as i32).min(fh - 1);
    let inside = ((x0 + x1) / 2, (y0 + y1) / 2);
    let (points, seed) = match g.below(if leak { 8 } else { 5 }) {
        0..=3 => (otl_rect(x0, y0, x1, y1), inside),
        4 => (otl_rect(x0, y0, x1, y1), (x0, y0)),
        5 => (
            otl_rect(x0, y0, x1, y1),
            (g.int(0, 511) as i32, g.int(0, 511) as i32),
        ),
        6 => {
            let r = g.int(2, 6) as i32;
            (otl_diamond(x0 + r, y0 + r, r), (x0 + r, y0 + r))
        }
        _ => {
            let n = g.int(0, 6);
            let pts = (0..n)
                .map(|_| {
                    (
                        g.int(0, i64::from(fw - 1)) as i32,
                        g.int(0, i64::from(fh - 1)) as i32,
                    )
                })
                .collect();
            (pts, inside)
        }
    };
    Outline {
        label: Some(g.pick(OTL_LABELS).to_owned()),
        seed,
        points,
        otype: None,
        extra: String::new(),
    }
}

/// An `.otl` file's bytes damaged: cut at a random byte, a line dropped, or a keyword changed.
fn otl_damage(g: &mut Gen, b: &mut Vec<u8>) {
    match g.below(3) {
        0 => {
            let at = g.below(b.len() as u64) as usize;
            b.truncate(at);
        }
        1 => {
            let lines: Vec<usize> = std::iter::once(0)
                .chain(
                    b.iter()
                        .enumerate()
                        .filter(|&(_, &c)| c == b'\n')
                        .map(|(i, _)| i + 1),
                )
                .filter(|&i| i < b.len())
                .collect();
            let a = g.pick(&lines);
            let e = b[a..]
                .iter()
                .position(|&c| c == b'\n')
                .map_or(b.len(), |p| a + p + 1);
            b.drain(a..e);
        }
        _ => {
            let keys: [&[u8]; 9] = [
                b"ROW_NUM",
                b"SEED",
                b"LABEL",
                b"END POINTS",
                b"START POINTS",
                b"END GDF",
                b"COL_NUM",
                b"ONUM",
                b"TYPE",
            ];
            let k = g.pick(&keys);
            if let Some(p) = b.windows(k.len()).position(|w| w == k) {
                b[p] = b'X';
            }
        }
    }
}

/// One seeded outline series: 1 to 3 slices (a slice's file sometimes missing or damaged), the
/// volume's size from a header-only `--in_like` (171 to 208 voxels and 256, a size below 171 or
/// above 512 now and then) or the default 512, outlines of every kind, the options of the read
/// (`-zo`, `-nt`, `-fp`, `-sp`) and an output format.
fn otl_case(g: &mut Gen) -> String {
    let k = g.int(1, 3) as i32;
    let (w, h, like) = match g.below(12) {
        0 => (512, 512, false),
        1 => {
            let s = g.pick(&[100, 128, 600]);
            (s, s, true)
        }
        2 | 3 => (256, 256, true),
        _ => (g.int(171, 208) as i32, g.int(171, 208) as i32, true),
    };
    let (fw, fh) = (2 * w, 2 * h);
    // a fill of the whole field is deep enough to crash FreeSurfer from 209 x 209 voxels on
    let leak = w <= 208 && h <= 208;
    let gt = g.pick(&[
        Some("ascii short"),
        Some("ascii short"),
        Some("short"),
        None,
    ]);
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    for s in 1..=k {
        if k > 1 && g.chance(5) {
            continue;
        }
        let n = g.int(0, 3);
        let mut outlines: Vec<Outline> = (0..n).map(|_| otl_outline(g, fw, fh, leak)).collect();
        for o in &mut outlines {
            if gt.is_none() || g.chance(6) {
                o.otype = Some(g.pick(&["ascii short", "short"]));
            }
            if g.chance(12) {
                o.label = None;
            }
        }
        let mut b = otl_bytes(&outlines, gt, None);
        if g.chance(6) {
            otl_damage(g, &mut b);
        }
        files.push((format!("in_{s}.otl"), b));
    }
    if files.is_empty() {
        files.push(("in_1.otl".into(), otl_bytes(&[], Some("ascii short"), None)));
    }
    let first = files[0].0.clone();
    files.push(("in_lut.txt".into(), OTL_LUT.to_vec()));
    let mut args: Vec<String> = Vec::new();
    if like {
        files.push(("in_l.mgh".into(), otl_like(w, h, k)));
        args.extend(["--in_like".into(), "in_l.mgh".into()]);
    } else {
        args.extend(["--in_k_count".into(), k.to_string()]);
    }
    args.extend(["-cf".into(), "in_lut.txt".into()]);
    for (o, p) in [("-zo", 4), ("-nt", 4), ("-fp", 3)] {
        if g.chance(p) {
            args.push(o.into());
        }
    }
    if g.chance(8) {
        args.extend(["-sp".into(), g.int(14, 26).to_string()]);
    }
    args.push(first);
    args.push(g.pick(&["o.mgh", "o.mgh", "o.mgz", "o.nii"]).into());
    cnv_line(&files, &args)
}

/// The outline conversions every seed starts with: each path of the reader once.
fn otl_prelude() -> Vec<String> {
    let lut = || ("in_lut.txt".to_owned(), OTL_LUT.to_vec());
    let like = |w: i32, h: i32, d: i32| ("in_l.mgh".to_owned(), otl_like(w, h, d));
    let rect = |label: &str, x0: i32, y0: i32, x1: i32, y1: i32| Outline {
        label: Some(label.to_owned()),
        seed: ((x0 + x1) / 2, (y0 + y1) / 2),
        points: otl_rect(x0, y0, x1, y1),
        otype: None,
        extra: String::new(),
    };
    let a = |xs: &[&str]| -> Vec<String> { xs.iter().map(|s| (*s).to_owned()).collect() };
    let std_args = |extra: &[&str]| -> Vec<String> {
        let mut v = a(&["--in_like", "in_l.mgh", "-cf", "in_lut.txt"]);
        v.extend(a(extra));
        v.extend(a(&["in_1.otl", "o.mgh"]));
        v
    };
    let two = || {
        vec![
            rect("Left Cerebral Cortex", 30, 30, 60, 50),
            rect("Right-Cerebral-White-Matter", 100, 120, 140, 150),
        ]
    };
    let file = |o: &[Outline]| otl_bytes(o, Some("ascii short"), None);
    let mut out = Vec::new();
    let mut push =
        |files: Vec<(String, Vec<u8>)>, args: Vec<String>| out.push(cnv_line(&files, &args));
    // the plain read: two slices, a volume of 200 x 190, and the default size (512)
    push(
        vec![
            ("in_1.otl".into(), file(&two())),
            ("in_2.otl".into(), file(&[rect("Mystery", 5, 5, 9, 9)])),
            lut(),
            like(200, 190, 2),
        ],
        std_args(&[]),
    );
    push(
        vec![("in_1.otl".into(), file(&two())), lut()],
        a(&[
            "--in_k_count",
            "1",
            "-cf",
            "in_lut.txt",
            "in_1.otl",
            "o.mgz",
        ]),
    );
    // the read's options
    for opts in [
        &["-zo"][..],
        &["-nt"],
        &["-fp"],
        &["-sp", "14"],
        &["-zo", "-nt", "-fp", "-sp", "26"],
    ] {
        let o = vec![
            rect("Left-Cerebral-Exterior", 20, 20, 80, 70),
            rect("Left-Cerebral-Cortex", 40, 30, 60, 50),
        ];
        push(
            vec![("in_1.otl".into(), file(&o)), lut(), like(180, 180, 1)],
            std_args(opts),
        );
    }
    // the header only (-ro with --in_info)
    push(
        vec![("in_1.otl".into(), file(&two())), lut(), like(180, 180, 1)],
        std_args(&["-ro", "-ii"]),
    );
    // types: binary points, a global type with an outline's own, none, an unsupported one
    let mut o = two();
    o[1].otype = Some("short");
    push(
        vec![
            ("in_1.otl".into(), otl_bytes(&o, Some("ascii short"), None)),
            lut(),
            like(180, 180, 1),
        ],
        std_args(&[]),
    );
    push(
        vec![
            ("in_1.otl".into(), otl_bytes(&two(), Some("short"), None)),
            lut(),
            like(180, 180, 1),
        ],
        std_args(&[]),
    );
    push(
        vec![
            ("in_1.otl".into(), otl_bytes(&two(), None, None)),
            lut(),
            like(180, 180, 1),
        ],
        std_args(&[]),
    );
    let mut o = two();
    o[0].otype = Some("float");
    push(
        vec![
            ("in_1.otl".into(), otl_bytes(&o, None, None)),
            lut(),
            like(180, 180, 1),
        ],
        std_args(&[]),
    );
    // the labels' names: each of the table's spellings, the unknown ones (more than 100: the list's
    // end), an empty label, entry 0, 5307 (FS-1055) and one past the claims' range
    let names: Vec<Outline> = OTL_LABELS
        .iter()
        .enumerate()
        .map(|(k, l)| {
            rect(
                l,
                10 + 16 * (k as i32 % 20),
                10 + 40 * (k as i32 / 20),
                20 + 16 * (k as i32 % 20),
                40 + 40 * (k as i32 / 20),
            )
        })
        .collect();
    push(
        vec![("in_1.otl".into(), file(&names)), lut(), like(200, 200, 1)],
        std_args(&[]),
    );
    push(
        vec![("in_1.otl".into(), file(&names)), lut(), like(200, 200, 1)],
        std_args(&["-nt"]),
    );
    let many: Vec<Outline> = (0..105)
        .map(|k| Outline {
            label: Some(format!("Unknown-{}", k % 103)),
            seed: (1, 1),
            points: vec![(0, 0)],
            otype: None,
            extra: String::new(),
        })
        .collect();
    push(
        vec![("in_1.otl".into(), file(&many)), lut(), like(180, 180, 1)],
        std_args(&[]),
    );
    let mut o = two();
    o[0].label = None;
    push(
        vec![("in_1.otl".into(), file(&o)), lut(), like(180, 180, 1)],
        std_args(&[]),
    );
    let o = vec![
        rect("Test-Unc", 10, 10, 30, 30),
        rect("Left-Cerebral-Cortex", 100, 100, 120, 130),
        rect("Beyond-Max", 200, 200, 220, 220),
    ];
    push(
        vec![
            ("in_1.otl".into(), file(&o)),
            ("in_2.otl".into(), file(&o)),
            lut(),
            like(180, 180, 2),
        ],
        std_args(&[]),
    );
    // more than ten claims on one place, an outline sharing points with another, entry 0
    let o: Vec<Outline> = (0..12)
        .map(|k| rect(OTL_LABELS[k % 4], 50, 50, 60 + k as i32, 70))
        .collect();
    push(
        vec![("in_1.otl".into(), file(&o)), lut(), like(180, 180, 1)],
        std_args(&[]),
    );
    // fills: a seed on the outline, outside it (the whole field but the outline), a leaking
    // diamond, no points, scattered points, a seed past the field, points past it and outside 512
    let mut o = two();
    o[0].seed = (30, 30);
    o[1].seed = (5, 300);
    push(
        vec![("in_1.otl".into(), file(&o)), lut(), like(190, 180, 1)],
        std_args(&[]),
    );
    let o = vec![
        Outline {
            label: Some("CSF".into()),
            seed: (20, 20),
            points: otl_diamond(20, 20, 5),
            otype: None,
            extra: String::new(),
        },
        Outline {
            label: Some("Left-Hippocampus".into()),
            seed: (0, 0),
            points: Vec::new(),
            otype: None,
            extra: String::new(),
        },
    ];
    push(
        vec![("in_1.otl".into(), file(&o)), lut(), like(171, 171, 1)],
        std_args(&[]),
    );
    let mut o = two();
    o[0].seed = (400, 10);
    o[1].points.push((380, 10));
    push(
        vec![("in_1.otl".into(), file(&o)), lut(), like(171, 171, 1)],
        std_args(&[]),
    );
    let mut o = two();
    o[1].points.push((600, 10));
    push(
        vec![("in_1.otl".into(), file(&o)), lut(), like(256, 256, 1)],
        std_args(&[]),
    );
    // a fill of the whole field of 420 x 420 places, deeper than the stack (FS-1144: FreeSurfer
    // crashes), and a field past the machine's memory (FS-1145)
    let none = |label: &str| Outline {
        label: Some(label.to_owned()),
        seed: (0, 0),
        points: Vec::new(),
        otype: None,
        extra: String::new(),
    };
    push(
        vec![
            ("in_1.otl".into(), file(&[none("Left-Cerebral-Cortex")])),
            lut(),
            like(210, 210, 1),
        ],
        std_args(&[]),
    );
    push(
        vec![
            ("in_1.otl".into(), file(&[none("CSF")])),
            lut(),
            like(10000, 10000, 1),
        ],
        std_args(&[]),
    );
    // sizes: a width that does not divide 512, one below 171 (FS-1057), above 512
    push(
        vec![("in_1.otl".into(), file(&two())), lut(), like(300, 200, 1)],
        std_args(&[]),
    );
    push(
        vec![("in_1.otl".into(), file(&two())), lut(), like(100, 128, 1)],
        std_args(&[]),
    );
    push(
        vec![("in_1.otl".into(), file(&two())), lut(), like(600, 180, 1)],
        std_args(&[]),
    );
    // the series: a missing slice, none found, a count of 0 (FS-1142), names without digits or
    // with digits only, no colour table, a colour table that cannot be read
    push(
        vec![
            ("in_1.otl".into(), file(&two())),
            ("in_3.otl".into(), file(&two())),
            lut(),
            like(180, 180, 3),
        ],
        std_args(&[]),
    );
    push(
        vec![("in_5.otl".into(), file(&two())), lut(), like(180, 180, 2)],
        a(&[
            "--in_like",
            "in_l.mgh",
            "-cf",
            "in_lut.txt",
            "in_5.otl",
            "o.mgh",
        ]),
    );
    push(
        vec![("in_1.otl".into(), file(&two())), lut()],
        a(&[
            "--in_k_count",
            "0",
            "-cf",
            "in_lut.txt",
            "in_1.otl",
            "o.mgh",
        ]),
    );
    push(
        vec![
            ("in_x.otl".into(), file(&two())),
            ("in_x1.otl".into(), file(&two())),
            lut(),
            like(180, 180, 1),
        ],
        a(&[
            "--in_like",
            "in_l.mgh",
            "-cf",
            "in_lut.txt",
            "in_x.otl",
            "o.mgh",
        ]),
    );
    push(
        vec![("12.otl".into(), file(&two())), lut(), like(180, 180, 1)],
        a(&[
            "--in_like",
            "in_l.mgh",
            "-cf",
            "in_lut.txt",
            "12.otl",
            "o.mgh",
        ]),
    );
    push(
        vec![("in_1.otl".into(), file(&two())), like(180, 180, 1)],
        a(&["--in_like", "in_l.mgh", "in_1.otl", "o.mgh"]),
    );
    push(
        vec![("in_1.otl".into(), file(&two())), like(180, 180, 1)],
        std_args(&[]),
    );
    push(
        vec![("in_1.otl".into(), file(&two())), lut()],
        a(&["-cf", "in_lut.txt", "in_1.otl", "o.mgh"]),
    );
    // the files' errors: not a GDF file (FS-1054), an empty one, each premature end, ONUM missing
    // or negative, ROW_NUM missing, COL_NUM not 2, a SEED of one number, START POINTS missing, a
    // point line short of numbers (FS-1141), numbers past a short, END POINTS missing, binary
    // points cut short, a line past the buffer, a LABEL line at the end of the file
    let good = file(&two());
    let cut_at = |key: &[u8]| -> Vec<u8> {
        let p = good
            .windows(key.len())
            .position(|w| w == key)
            .expect("the key");
        good[..p + key.len()].to_vec()
    };
    let replace = |from: &str, to: &str| -> Vec<u8> {
        String::from_utf8(good.clone())
            .expect("ascii")
            .replacen(from, to, 1)
            .into_bytes()
    };
    let bad: Vec<Vec<u8>> = vec![
        b"hello\n".to_vec(),
        Vec::new(),
        b"GDF FILE VERSION 1\n".to_vec(),
        cut_at(b"START MAIN HEADER\n"),
        cut_at(b"END MAIN HEADER\n"),
        cut_at(b"START GDF HEADER\n"),
        cut_at(b"SEED"),
        cut_at(b"START POINTS\n"),
        cut_at(b"START POINTS"),
        cut_at(b"30 50\n"),
        cut_at(b"END POINTS"),
        replace("ONUM 2\n", ""),
        replace("ONUM 2\n", "ONUM -3\n"),
        replace("ONUM 2\n", "ONUM 3\n"),
        replace("ONUM 2\n", "ONUM 1\n"),
        replace("ROW_NUM", "ROW_NUMBER_X"),
        replace("ROW_NUM 100", "ROW_NUM -1"),
        replace("COL_NUM 2\n", "COL_NUM 3\n"),
        replace("SEED 45 40", "SEED 45"),
        replace("SEED 45 40", "SEED 45 512"),
        replace("START POINTS", "START PTS"),
        replace("30 30\n", "30\n"),
        replace("30 30\n", "x y\n"),
        replace("30 30\n", "65566 -65506\n"),
        replace("END POINTS", "END PTS"),
        replace(
            "LABEL Left Cerebral Cortex\n",
            &format!("LABEL {}\n", "L".repeat(1500)),
        ),
        {
            let mut b = good.clone();
            b.extend_from_slice(b"START GDF HEADER\nROW_NUM 1\nLABEL");
            b
        },
        replace("COL_NUM 2\n", "COL_NUM 2\nTYPE\n"),
    ];
    for b in bad {
        push(
            vec![("in_1.otl".into(), b), lut(), like(180, 180, 1)],
            std_args(&[]),
        );
    }
    let mut o = two();
    o[1].otype = Some("short");
    let mut b = otl_bytes(&o, Some("ascii short"), None);
    let p = b
        .windows(10)
        .rposition(|w| w == b"END POINTS")
        .expect("the end");
    b.drain(p - 7..p);
    push(
        vec![("in_1.otl".into(), b), lut(), like(180, 180, 1)],
        std_args(&[]),
    );
    // the warning of an exterior label after others (FS-1060) and the translation's cut
    let o = vec![
        rect("Left-Hippocampus", 10, 10, 20, 20),
        rect("Left-Cerebral-Exterior-Extra", 30, 30, 50, 50),
        rect("Exterior and cortex, Cortex and exterior", 60, 60, 70, 70),
        rect("CSF", 80, 80, 90, 90),
    ];
    push(
        vec![("in_1.otl".into(), file(&o)), lut(), like(180, 180, 1)],
        std_args(&[]),
    );
    push(
        vec![("in_1.otl".into(), file(&o)), lut(), like(180, 180, 1)],
        std_args(&["-nt"]),
    );
    // the read through mri_convert's later steps: a type and a resampling
    push(
        vec![("in_1.otl".into(), file(&two())), lut(), like(180, 180, 1)],
        std_args(&["-odt", "uchar"]),
    );
    push(
        vec![("in_1.otl".into(), file(&two())), lut(), like(180, 180, 1)],
        std_args(&["-vs", "2", "2", "2"]),
    );
    out
}

/// `n` outline conversions of seed `seed`: the [`otl_prelude`], then seeded ones.
pub fn generate_otl(seed: u64, n: usize) -> Vec<String> {
    let mut v = otl_prelude();
    v.truncate(n);
    let mut g = Gen::new(seed ^ 0x6f74_6c00);
    while v.len() < n {
        v.push(otl_case(&mut g));
    }
    v
}
