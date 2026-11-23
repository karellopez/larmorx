// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_concat`'s `parse_commandline` (`mri_concat.cpp` 777-1201), `check_options` (1305-1366),
//! `usage_exit`, `print_usage`, `print_help`, `print_version` and `argnerr` (1203-1303).
//!
//! The options are read one by one; an argument that is no option is an input (any argument
//! FreeSurfer does not know, `--foo` included). `--help` the usage and the help (status 1),
//! `--version` the version (status 1); an option missing its arguments `ERROR: <option> flag needs
//! <n> argument(s)` on standard error, status 255. The flags `--debug`, `--check`, `--no-check`,
//! the reductions and the other switches match in any case (`strcasecmp`), the options taking
//! values exactly (`strcmp`) but for `--max-index-add`, `--ctab`, `--tar1` and `--f`. Numbers are
//! read with `sscanf` (a value that does not read leaves the setting as it was); `--max-index-add`,
//! `--mul`, `--div` and `--add` first refuse a value whose first character is not a digit, a sign
//! (or a point, for `--mul` and `--div`), status 1. `--mtx`, `--w` and `--wn` read their matrix
//! (`MatrixReadTxt`; `ERROR: reading <file>`, status 1), `--ctab` its colour table (status 1 when
//! it does not read), at once. `--zconcat <bot> <top> <nskip> <out>` does its work at once
//! (`MRIzconcat`) and ends the program with the writer's status. `--chunk` and `--no-chunk` set and
//! clear `FS_USE_MRI_CHUNK`, which no code of FreeSurfer 8.2.0 reads (no effect).
//!
//! `--f <list>`: the first word of the argument (`sscanf("%s")`) names a text file whose words
//! (`fscanf("%s")`, to its end; a file that cannot be read gives none) become inputs; `ERROR:
//! opening <name>`, status 1, when it does not open. FreeSurfer keeps the words in a table of
//! 400,000 slots of 2,000 bytes: FS-1194 (a word of 2,000 bytes or more runs into the next slots),
//! FS-1195 (the argument's first word only), FS-1196 (an argument without a word), FS-1197 (a name
//! of 2,000 bytes or more), FS-1198 (more than 400,000 inputs).


use larmorx_core::defects::{FS_1194, FS_1195, FS_1196, FS_1197, FS_1198, FS_1247};
use larmorx_freesurfer_core::cmdargs::cmd_arg_n_err;
use larmorx_freesurfer_core::cscan::{Bytes, Scanf};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::Matrix;
use larmorx_freesurfer_core::matrix_io::matrix_read_txt;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::version::get_version;
use larmorx_freesurfer_io::cfile::CFile;
use larmorx_freesurfer_io::colortab::ctab_read_ascii;
use larmorx_freesurfer_io::mriio::{mri_read_volume, mri_write};
use larmorx_freesurfer_mri::ctab::ColorTable;
use larmorx_freesurfer_mri::mri::MGZ_INTENT_UNKNOWN;
use larmorx_freesurfer_volume::mri2ops::mri_zconcat;

use super::main::Run;
use crate::mri_segstats::options::{scan_d, scan_lf};

/// `NInMAX`: the inputs' table.
pub const NIN_MAX: usize = 400_000;
/// The size of a list file's word slot (`flist[NInMAX][2000]`) and of `listfile[2000]`.
pub const SLOT: usize = 2000;

/// The settings the options give (`mri_concat.cpp`'s globals).
#[derive(Default)]
pub struct Options {
    pub debug: bool,
    pub inputs: Vec<String>,
    pub out: Option<String>,
    pub maskfile: Option<String>,
    pub do_mean: bool,
    pub do_norm_mean: bool,
    pub do_norm1: bool,
    pub do_mean_div_n: bool,
    pub do_median: bool,
    pub do_sum: bool,
    pub do_var: bool,
    pub do_std: bool,
    pub do_max: bool,
    pub do_max_index: bool,
    pub do_max_index_prune: bool,
    pub do_max_index_add: bool,
    pub max_index_add: i32,
    pub do_min: bool,
    pub do_conjunction: bool,
    pub do_paired: bool,
    pub do_paired_avg: bool,
    pub do_paired_sum: bool,
    pub do_paired_diff: bool,
    pub do_paired_diff_norm: bool,
    pub do_paired_diff_norm1: bool,
    pub do_paired_diff_norm2: bool,
    pub do_asl: bool,
    pub do_vote: bool,
    pub vote_exclude0: bool,
    pub do_first_non_zero: bool,
    pub do_sort: bool,
    pub do_combine: bool,
    pub do_keep_datatype: bool,
    pub do_multiply: bool,
    pub multiply_val: f64,
    pub do_add: bool,
    pub add_val: f64,
    pub do_bonf_cor: bool,
    pub do_abs: bool,
    pub do_pos: bool,
    pub do_neg: bool,
    pub in_frame_no: i32,
    /// `M` (`--mtx`, replaced by `--gmean`'s and `--asl`'s in `main`).
    pub m: Option<Matrix>,
    /// `FrameWeight` (`--w`, `--wn`).
    pub frame_weight: Option<Matrix>,
    pub ngroups: i32,
    pub do_pca: bool,
    pub pca_mask_file: Option<String>,
    pub do_scm: bool,
    pub do_check: bool,
    pub do_tar1: bool,
    pub tar1_dof_adjust: i32,
    pub n_replications: i32,
    pub do_prune: bool,
    pub do_rms: bool,
    pub do_cum_sum: bool,
    pub do_fnorm: bool,
    pub rusage_file: Option<String>,
    pub ctab: Option<ColorTable>,
    /// FS-1197 (`reproduce`): a list file name passed the parser's buffer and the file opened.
    pub stack_smashed: bool,
}

impl Options {
    fn new() -> Options {
        Options {
            do_check: true,
            in_frame_no: -1,
            tar1_dof_adjust: 1,
            ..Options::default()
        }
    }
}

/// `print_usage()`.
pub fn print_usage(r: &mut Run) {
    let n = NIN_MAX;
    let text = format!(
        "USAGE: {name} \n\n   --o out \n   --i invol <--i invol ...> (don't need --i) \n   --f listfile : list file has a text list of files (up to {n})\n   --in-frame frameno : extract frameno from the input\n\n   --paired-sum  : compute paired sum (1+2, 3d+4, etc) \n   --paired-avg  : compute paired avg (1+2, 3d+4, etc) \n   --paired-diff : compute paired diff (1-2, 3-4, etc) \n   --paired-diff-norm : same as paired-diff but scale by TP1,2 average \n   --paired-diff-norm1 : same as paired-diff but scale by TP1 \n   --paired-diff-norm2 : same as paired-diff but scale by TP2 \n   --norm-mean         : normalize frames by mean of all TP\n   --norm1             : normalize frames by TP1 \n   --mtx matrix.asc    : multiply by matrix in ascii file\n   --w frameweight.asc : weight each frame by values in ascii file (one val for each frame)\n   --wn frameweight.asc : same as --w but normalizes the frames to sum to 1\n   --gmean Ng          : create matrix to average Ng groups, Nper=Ntot/Ng\n\n   --combine : average frames from non-zero voxels\n             (useful to combine lh.ribbon.mgz and rh.ribbon.mgz)\n   --keep-datatype : write output in same datatype as input\n                    (default is to write output in Float format)\n\n   --abs  : take abs of input\n   --pos  : set input negatives to 0\n   --neg  : set input postives to 0\n   --mean : compute mean of concatenated volumes\n   --median : compute median of concatenated volumes\n   --mean-div-n : compute mean/nframes (good for var) \n   --sum  : compute sum of concatenated volumes\n   --var  : compute var  of concatenated volumes\n   --std  : compute std  of concatenated volumes\n   --max  : compute max  of concatenated volumes\n   --max-index  : compute index of max of concatenated volumes (1-based)\n   --max-index-prune  : max index setting to 0 any voxel where all frames are 0 (not the same as --prune)\n   --max-index-add val  : add val to non-zero max indices)\n   --min  : compute min of concatenated volumes\n   --rep N : replicate N times (over frame)\n   --fnorm : normalize time series at each voxel (remove mean, divide by SSS)\n   --conjunct  : compute voxel-wise conjunction concatenated volumes\n   --vote : most frequent value at each voxel and fraction of occurances\n   --first-non-zero : return the first non-zero value across frames\n   --vote-ex0 : most frequent value at each voxel excluding 0 and fraction of occurances\n   --ctab ctab : embed color table in the output\n   --sort : sort each voxel by ascending frame value\n   --tar1 dofadjust : compute temporal ar1\n   --prune : set vox to 0 unless all frames are non-zero\n   --pca  : output is pca. U is output.u.mtx and S is output.stats.dat\n   --pca-mask mask  : Only use voxels whose mask > 0.5\n   --scm  : compute spatial covariance matrix (can be huge!)\n   --zconcat bot top nskip out : concat in the slice direction skipping nskip slices of the top\n\n   --max-bonfcor  : compute max and bonferroni correct (assumes -log10(p))\n   --mul mulval   : multiply by mulval\n   --div divval   : divide by divval\n   --add addval   : add addval\n\n   --mask maskfile : mask used with --vote or --sort\n   --rms : root mean square (eg. combine memprage)\n           (square, sum, div-by-nframes, square root)\n   --no-check : do not check inputs (faster)\n   --help      print out information on how to use this program\n   --version   print out version and exit\n\n{version}\n\n",
        name = super::main::NAME,
        version = get_version(),
    );
    let _ = r.out.write_all(text.as_bytes());
}

/// `usage_exit()`: the usage, status 1.
pub fn usage_exit(r: &mut Run) -> Fatal {
    print_usage(r);
    Fatal::Exit(1)
}

/// `print_help()`: the usage and the help, status 1.
fn print_help(r: &mut Run) -> Fatal {
    print_usage(r);
    let _ = r.out.write_all(
        b"Concatenates input data sets.\nEXAMPLES:\n  mri_concat --i f1.mgh --i f2.mgh --o cout.mgh\n  mri_concat f1.mgh f2.mgh --o cout.mgh\n  mri_concat f*.mgh --o cout.mgh\n  mri_concat f*.mgh --o coutmn.mgh --mean\n  mri_concat f*.mgh --o coutdiff.mgh --paired-diff\n  mri_concat f*.mgh --o coutdiff.mgh --paired-diff-norm\n  mri_concat f*.mgh --o coutdiff.mgh --paired-diff-norm1\n\nConjunction takes the min of the abs across frames\nat each voxel. The output value at the voxel is the min, \nincluding the true sign of the min. Eg, if the two frames are:\n   +2.1 and +3.4 --> +2.1\n   -2.1 and -3.4 --> -2.1\n   +2.1 and -3.4 --> +2.1\n   -2.1 and +3.4 --> -2.1\nSee: Nichols, Brett,Andersson, Wager, and Poline\nNeuroImage, Volume 25, Issue 3, 15 April 2005, 653-660.\n",
    );
    Fatal::Exit(1)
}

/// `print_version()`: the version, status 1.
fn print_version(r: &mut Run) -> Fatal {
    let _ = writeln!(r.out, "{}", get_version());
    Fatal::Exit(1)
}

/// Whether a value passes `--max-index-add`'s and `--add`'s check (its first character a digit
/// or a sign; `point`: or a point, `--mul` and `--div`).
fn numberish(s: &str, point: bool) -> bool {
    match s.as_bytes().first() {
        Some(c) => c.is_ascii_digit() || *c == b'-' || *c == b'+' || (point && *c == b'.'),
        None => false,
    }
}

/// `sscanf(s, "%d", &v)` into `v` (left as it was when none is read).
fn scan_d_into(g: &mut Globals, s: &str, v: &mut i32) {
    if let Some(x) = scan_d(g, s) {
        *v = x;
    }
}

/// `sscanf(s, "%lf", &v)` into `v`.
fn scan_lf_into(g: &mut Globals, s: &str, v: &mut f64) {
    if let Some(x) = scan_lf(g, s) {
        *v = x;
    }
}

/// `MatrixReadTxt(name, NULL)` for `--mtx`, `--w` and `--wn`: the matrix, or `ERROR: reading
/// <name>` and status 1.
fn read_matrix(g: &mut Globals, r: &mut Run, name: &str) -> Result<Matrix, Fatal> {
    match matrix_read_txt(g, r.out, r.err, r.dir, name)? {
        Ret::New(m) => Ok(m),
        _ => {
            let _ = writeln!(r.out, "ERROR: reading {name}");
            Err(Fatal::Exit(1))
        }
    }
}

/// The words of `--f`'s list files, in the order they were read (with their places in the
/// inputs): FreeSurfer's table of 2,000-byte slots, read when the inputs are used.
#[derive(Default)]
struct ListWords {
    /// (the input's index, the word)
    words: Vec<(usize, Vec<u8>)>,
}

impl ListWords {
    /// The names the inputs read from the table: a word of 2,000 bytes or more has no end in its
    /// slot and runs on into the next slot's name (FS-1194, `reproduce`); by default each word is
    /// its own name.
    fn names(&self, g: &mut Globals) -> Vec<(usize, String)> {
        let n = self.words.len();
        let long = self.words.iter().position(|(_, w)| w.len() >= SLOT);
        if let Some(k) = long {
            g.notices.add(
                &FS_1194,
                if g.notices.reproduce() {
                    format!(
                        "mri_concat --f: a word of {} bytes runs into the next words' slots of FreeSurfer's table (2,000 bytes each), as in FreeSurfer",
                        self.words[k].1.len()
                    )
                } else {
                    format!(
                        "mri_concat --f: a word of {} bytes used whole (FreeSurfer's table holds 1,999 bytes a word: its name runs into the next words)",
                        self.words[k].1.len()
                    )
                },
            );
        }
        let mut names: Vec<Vec<u8>> = vec![Vec::new(); n];
        for k in (0..n).rev() {
            let w = &self.words[k].1;
            names[k] = if g.notices.reproduce() && w.len() >= SLOT && k + 1 < n {
                let mut v = w[..SLOT].to_vec();
                v.extend_from_slice(&names[k + 1]);
                v
            } else {
                w.clone()
            };
        }
        self.words
            .iter()
            .zip(names)
            .map(|((i, _), b)| (*i, String::from_utf8_lossy(&b).into_owned()))
            .collect()
    }
}

/// `--f <list>`: the list file's words appended to the inputs (and to the table's words).
fn read_list(
    g: &mut Globals,
    r: &mut Run,
    o: &mut Options,
    table: &mut ListWords,
    arg: &str,
) -> Result<(), Fatal> {
    // sscanf(pargv[0], "%s", listfile): the argument's first word
    let mut b = Bytes::new(arg.as_bytes());
    let mut sc = Scanf::new(&mut b);
    let word = sc.s(&mut g.errno, usize::MAX, true);
    let assigned = sc.finish(&mut g.errno) == 1;
    let listfile = match word {
        Some(w) if assigned => {
            let w = String::from_utf8_lossy(&w).into_owned();
            if w != arg {
                g.notices.add(
                    &FS_1195,
                    if g.notices.reproduce() {
                        format!(
                            "mri_concat --f '{arg}': the argument's first word '{w}' opened, as in FreeSurfer"
                        )
                    } else {
                        format!(
                            "mri_concat --f '{arg}': the whole argument opened (FreeSurfer opens its first word '{w}')"
                        )
                    },
                );
                if g.notices.reproduce() { w } else { arg.to_owned() }
            } else {
                w
            }
        }
        _ => {
            g.notices.add(
                &FS_1196,
                if g.notices.reproduce() {
                    "mri_concat --f: an argument without a word leaves FreeSurfer's name buffer as it was (memory contents): opened as an empty name".to_owned()
                } else {
                    "mri_concat --f: an argument without a word opened as an empty name (FreeSurfer opens what its buffer holds: memory contents)".to_owned()
                },
            );
            arg.to_owned()
        }
    };
    if listfile.len() >= SLOT {
        g.notices.add(
            &FS_1197,
            format!(
                "mri_concat --f: a list file name of {} bytes used whole (FreeSurfer's buffer holds 1,999: the name passes it, and the official build stops when its parsing ends if the file opened)",
                listfile.len()
            ),
        );
    }
    let mut fp = match CFile::open(r.dir, &listfile, "r") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            let _ = writeln!(r.out, "ERROR: opening {listfile}");
            return Err(Fatal::Exit(1));
        }
    };
    if listfile.len() >= SLOT && g.notices.reproduce() {
        // the overflow reached the stack's guard: the official build aborts at the parser's end
        o.stack_smashed = true;
    }
    loop {
        let mut sc = Scanf::new(&mut fp);
        let w = sc.s(&mut g.errno, usize::MAX, true);
        let rt = sc.finish(&mut g.errno);
        if rt == -1 {
            break;
        }
        let w = w.unwrap_or_default();
        table.words.push((o.inputs.len(), w));
        o.inputs.push(String::new());
    }
    fp.close();
    Ok(())
}

/// `parse_commandline(argc, argv)` on the arguments after the program's name.
pub fn parse_commandline(
    g: &mut Globals,
    r: &mut Run,
    args: &[String],
) -> Result<Options, Fatal> {
    let mut o = Options::new();
    let mut table = ListWords::default();
    if args.is_empty() {
        return Err(usage_exit(r));
    }
    let mut at = 0usize;
    while at < args.len() {
        let option = args[at].as_str();
        let nargc = (args.len() - at) as i32;
        if o.debug {
            let _ = writeln!(r.out, "{nargc} {option}");
        }
        at += 1;
        let rest = &args[at..];
        let mut nargsused = 0usize;
        let ci = |s: &str| option.eq_ignore_ascii_case(s);
        macro_rules! argn {
            ($n:expr) => {
                if rest.len() < $n {
                    return Err(cmd_arg_n_err(r.err, option, $n));
                }
            };
        }
        if ci("--help") {
            return Err(print_help(r));
        } else if ci("--version") {
            return Err(print_version(r));
        } else if ci("--debug") {
            o.debug = true;
        } else if ci("--check") {
            o.do_check = true;
        } else if ci("--no-check") {
            o.do_check = false;
        } else if ci("--mean") {
            o.do_mean = true;
        } else if ci("--median") {
            o.do_median = true;
        } else if ci("--mean-div-n") || ci("--mean2") {
            o.do_mean_div_n = true;
        } else if ci("--sum") {
            o.do_sum = true;
        } else if ci("--std") {
            o.do_std = true;
        } else if ci("--var") {
            o.do_var = true;
        } else if ci("--abs") {
            o.do_abs = true;
        } else if ci("--fnorm") {
            o.do_fnorm = true;
        } else if ci("--pos") {
            o.do_pos = true;
        } else if ci("--neg") {
            o.do_neg = true;
        } else if ci("--max") {
            o.do_max = true;
        } else if ci("--max-index") {
            o.do_max_index = true;
        } else if ci("--max-index-prune") {
            o.do_max_index = true;
            o.do_max_index_prune = true;
        } else if ci("--max-index-add") {
            argn!(1);
            if !numberish(&rest[0], false) {
                let _ = r.out.write_all(
                    b"ERROR: value passed to the --max-index-add flag must be a number\n       If you want to add two images, use --sum or fscalc\n",
                );
                return Err(Fatal::Exit(1));
            }
            scan_d_into(g, &rest[0], &mut o.max_index_add);
            o.do_max_index_add = true;
            nargsused = 1;
        } else if ci("--min") {
            o.do_min = true;
        } else if ci("--conjunct") {
            o.do_conjunction = true;
        } else if ci("--vote") {
            o.do_vote = true;
        } else if ci("--vote-ex0") {
            o.do_vote = true;
            o.vote_exclude0 = true;
        } else if ci("--first-non-zero") {
            o.do_first_non_zero = true;
        } else if ci("--sort") {
            o.do_sort = true;
        } else if ci("--norm-mean") {
            o.do_norm_mean = true;
        } else if ci("--norm1") {
            o.do_norm1 = true;
        } else if ci("--prune") {
            o.do_prune = true;
        } else if ci("--no-prune") {
            o.do_prune = false;
        } else if ci("--max-bonfcor") {
            o.do_max = true;
            o.do_bonf_cor = true;
        } else if ci("--asl") {
            o.do_asl = true;
        } else if ci("--paired-avg") {
            o.do_paired = true;
            o.do_paired_avg = true;
        } else if ci("--paired-sum") {
            o.do_paired = true;
            o.do_paired_sum = true;
        } else if ci("--paired-diff") {
            o.do_paired = true;
            o.do_paired_diff = true;
        } else if ci("--paired-diff-norm") {
            o.do_paired_diff = true;
            o.do_paired_diff_norm = true;
            o.do_paired = true;
        } else if ci("--paired-diff-norm1") {
            o.do_paired_diff = true;
            o.do_paired_diff_norm1 = true;
            o.do_paired = true;
        } else if ci("--paired-diff-norm2") {
            o.do_paired_diff = true;
            o.do_paired_diff_norm2 = true;
            o.do_paired = true;
        } else if ci("--combine") {
            o.do_combine = true;
        } else if ci("--cumsum") {
            o.do_cum_sum = true;
        } else if ci("--rms") {
            o.do_rms = true;
        } else if ci("--keep-datatype") {
            o.do_keep_datatype = true;
        } else if ci("--pca") {
            o.do_pca = true;
        } else if ci("--chunk") || ci("--no-chunk") {
            // FS_USE_MRI_CHUNK: read by no code of FreeSurfer 8.2.0
        } else if ci("--scm") {
            o.do_scm = true;
        } else if option == "--pca-mask" {
            argn!(1);
            o.pca_mask_file = Some(rest[0].clone());
            nargsused = 1;
        } else if option == "--i" {
            argn!(1);
            push_input(g, r, &mut o, &table, rest[0].clone())?;
            nargsused = 1;
        } else if option == "--mtx" {
            argn!(1);
            o.m = Some(read_matrix(g, r, &rest[0])?);
            nargsused = 1;
        } else if option == "--w" || option == "--wn" {
            argn!(1);
            let mut w = read_matrix(g, r, &rest[0])?;
            if option == "--wn" {
                let _ = r.out.write_all(b"Normalizing frame weights to sum to 1\n");
                let mut sum = 0.0f64;
                for n in 1..=w.rows {
                    sum = larmorx_freesurfer_core::sse::add64(sum, f64::from(w.relt(n, 1)));
                }
                for n in 1..=w.rows {
                    let v = larmorx_freesurfer_core::sse::div64(f64::from(w.relt(n, 1)), sum);
                    w.set_relt(n, 1, v as f32);
                }
            }
            o.frame_weight = Some(w);
            nargsused = 1;
        } else if option == "--gmean" {
            argn!(1);
            scan_d_into(g, &rest[0], &mut o.ngroups);
            nargsused = 1;
        } else if option == "--in-frame" {
            argn!(1);
            scan_d_into(g, &rest[0], &mut o.in_frame_no);
            nargsused = 1;
        } else if option == "--rep" {
            argn!(1);
            scan_d_into(g, &rest[0], &mut o.n_replications);
            nargsused = 1;
        } else if option == "--o" {
            argn!(1);
            o.out = Some(rest[0].clone());
            nargsused = 1;
        } else if option == "--rusage" {
            argn!(1);
            o.rusage_file = Some(rest[0].clone());
            nargsused = 1;
        } else if option == "--mask" {
            argn!(1);
            o.maskfile = Some(rest[0].clone());
            nargsused = 1;
        } else if option == "--mul" || option == "--div" {
            argn!(1);
            if !numberish(&rest[0], true) {
                let _ = r.out.write_all(
                    b"ERROR: value passed to the --mul or or --div option must be a number\n       If you want to multiply two images, use fscalc\n",
                );
                return Err(Fatal::Exit(1));
            }
            scan_lf_into(g, &rest[0], &mut o.multiply_val);
            if option == "--div" {
                o.multiply_val = larmorx_freesurfer_core::sse::div64(1.0, o.multiply_val);
            }
            o.do_multiply = true;
            nargsused = 1;
        } else if option == "--zconcat" {
            argn!(4);
            return Err(zconcat(g, r, &rest[..4]));
        } else if option == "--add" {
            argn!(1);
            if !numberish(&rest[0], false) {
                let _ = r.out.write_all(
                    b"ERROR: value passed to the --add flag must be a number\n       If you want to add two images, use --sum or fscalc\n",
                );
                return Err(Fatal::Exit(1));
            }
            scan_lf_into(g, &rest[0], &mut o.add_val);
            o.do_add = true;
            nargsused = 1;
        } else if ci("--ctab") {
            argn!(1);
            match ctab_read_ascii(g, r.out, r.err, r.dir, &rest[0]) {
                Some(t) => o.ctab = Some(t),
                None => return Err(Fatal::Exit(1)),
            }
            nargsused = 1;
        } else if ci("--tar1") {
            argn!(1);
            scan_d_into(g, &rest[0], &mut o.tar1_dof_adjust);
            o.do_tar1 = true;
            nargsused = 1;
        } else if ci("--f") {
            argn!(1);
            let arg = rest[0].clone();
            read_list(g, r, &mut o, &mut table, &arg)?;
            if o.inputs.len() > NIN_MAX {
                g.notices.add(
                    &FS_1198,
                    format!(
                        "mri_concat --f: {} inputs: ERROR: ninputs > 400000, status 1 (FreeSurfer's tables hold 400,000: it writes past them, memory contents)",
                        o.inputs.len()
                    ),
                );
                let _ = writeln!(r.out, "ERROR: ninputs > {NIN_MAX}");
                return Err(Fatal::Exit(1));
            }
            nargsused = 1;
        } else {
            push_input(g, r, &mut o, &table, option.to_owned())?;
        }
        at += nargsused;
    }
    for (i, name) in table.names(g) {
        o.inputs[i] = name;
    }
    if o.stack_smashed {
        let _ = r.err.write_all(b"*** stack smashing detected ***: terminated\n");
        return Err(Fatal::Exit(134));
    }
    Ok(o)
}

/// An input from the command line: `ERROR: ninputs > 400000`, status 1, past the table.
fn push_input(
    _g: &mut Globals,
    r: &mut Run,
    o: &mut Options,
    _table: &ListWords,
    name: String,
) -> Result<(), Fatal> {
    o.inputs.push(name);
    if o.inputs.len() > NIN_MAX {
        let _ = writeln!(r.out, "ERROR: ninputs > {NIN_MAX}");
        return Err(Fatal::Exit(1));
    }
    Ok(())
}

/// `--zconcat <bot> <top> <nskip> <out>`: `MRIzconcat` of the two volumes into `out`, then the
/// program's end with the writer's status. A `nskip` that does not read is FS-1247.
fn zconcat(g: &mut Globals, r: &mut Run, a: &[String]) -> Fatal {
    let go = |g: &mut Globals, r: &mut Run| -> Result<Fatal, Fatal> {
        let Some(mri1) = mri_read_volume(g, r.out, r.err, &r.env, r.dir, &a[0])? else {
            return Ok(Fatal::Exit(1));
        };
        let Some(mri2) = mri_read_volume(g, r.out, r.err, &r.env, r.dir, &a[1])? else {
            return Ok(Fatal::Exit(1));
        };
        let nskip = match scan_d(g, &a[2]) {
            Some(n) => n,
            None => {
                g.notices.add(
                    &FS_1247,
                    if g.notices.reproduce() {
                        format!(
                            "mri_concat --zconcat: nskip '{}' is not a number: FreeSurfer's is memory contents (filled with 0)",
                            a[2]
                        )
                    } else {
                        format!(
                            "mri_concat --zconcat: nskip '{}' is not a number: ERROR, status 1 (FreeSurfer's is memory contents)",
                            a[2]
                        )
                    },
                );
                if g.notices.fix() {
                    let _ = writeln!(
                        r.out,
                        "ERROR: --zconcat: nskip {} is not a number",
                        a[2]
                    );
                    return Ok(Fatal::Exit(1));
                }
                0
            }
        };
        let mut out = match mri_zconcat(g, r.out, r.err, &mri1, &mri2, nskip, None)? {
            Ret::New(m) => m,
            _ => return Ok(Fatal::Exit(1)),
        };
        let err = mri_write(
            g,
            r.out,
            r.err,
            &r.env,
            r.dir,
            &mut out,
            &a[3],
            None,
            MGZ_INTENT_UNKNOWN,
        )?;
        Ok(Fatal::Exit(err))
    };
    match go(g, r) {
        Ok(f) | Err(f) => f,
    }
}

/// `check_options()`: the inputs and output given, the combinations refused (status 1). The
/// mask's check (`--mask` only with `--vote` or `--sort`) is made before the mask is read, so it
/// never refuses: kept, as the mask also serves `--first-non-zero` and `--cumsum`.
pub fn check_options(r: &mut Run, o: &Options) -> Result<(), Fatal> {
    let refuse = |r: &mut Run, msg: &str| -> Result<(), Fatal> {
        let _ = writeln!(r.out, "{msg}");
        Err(Fatal::Exit(1))
    };
    if o.inputs.is_empty() {
        return refuse(r, "ERROR: no inputs specified");
    }
    if o.out.is_none() {
        return refuse(r, "ERROR: no output specified");
    }
    if o.do_paired_diff && o.do_paired_avg {
        return refuse(r, "ERROR: cannot specify both --paried-diff-xxx and --paried-avg ");
    }
    if o.do_paired_diff_norm1 && o.do_paired_diff_norm2 {
        return refuse(
            r,
            "ERROR: cannot specify both --paried-diff-norm1 and --paried-diff-norm2 ",
        );
    }
    if o.do_paired_diff_norm && o.do_paired_diff_norm1 {
        return refuse(
            r,
            "ERROR: cannot specify both --paried-diff-norm and --paried-diff-norm1 ",
        );
    }
    if o.do_paired_diff_norm && o.do_paired_diff_norm2 {
        return refuse(
            r,
            "ERROR: cannot specify both --paried-diff-norm and --paried-diff-norm2 ",
        );
    }
    if o.do_mean && o.do_std {
        return refuse(r, "ERROR: cannot --mean and --std");
    }
    // `if(mask && !DoVote && !DoSort)`: the mask is read after this check, which never refuses
    // (the mask is used by --vote, --sort, --first-non-zero and --cumsum, ignored by the others)
    if o.do_std && o.do_var {
        return refuse(r, "ERROR: cannot compute std and var, you bonehead.");
    }
    if u8::from(o.do_abs) + u8::from(o.do_pos) + u8::from(o.do_neg) > 1 {
        return refuse(r, "ERROR: do not use more than one of --abs, --pos, --neg");
    }
    Ok(())
}
