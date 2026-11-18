// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_convert`'s command line (queue item 3.2-05): `main`'s start, its option loop and checks
//! (`mri_convert.cpp` 233-1874) with `get_ints`, `get_floats`, `get_string`, `usage_message` and
//! `usage` (3860-4004), the file statics (57-73) and the DICOM reader's settings the options set
//! (`DICOMRead.h`'s globals). [`parse`] returns everything the options and checks set
//! ([`Options`]), or the end of the program.
//!
//! The options are compared exactly (with their case), one per argument; an argument that starts
//! with `-` and is no option ends the program (`unknown flag`); the others are the input and the
//! output volume, a third ends it (`extra argument`). An option that takes integers, numbers or a
//! name checks it has them (`argument <opt> expects <n> integers; only <k> arguments after flag`,
//! `... floats ...`, `... expects an extra argument; none found`), that none is empty (`argument to
//! <opt> flag is null`) and that each is a whole integer (`strtol`, base 10, the `long` cut to an
//! `int`) or number (`strtod`, then a `float`); each such error prints the hint `type mri_convert
//! -u for usage` on standard output and ends with status 1.
//!
//! Defects (`larmorx_core::defects`; the mechanisms in `docs/findings/freesurfer-convert.md`):
//! - **FS-1038** (crash): `--rescale`, `-sc`/`--scale`, `-osc`/`--out-scale`, `--rescale-voxel`
//!   and `-dcm2niix-opts` read their arguments without checking there are any: both modes end
//!   with the message `get_floats`, `get_ints` or `get_string` gives for that case (status 1).
//! - **FS-1039** (memory): a name of 1,024 bytes or more (2,000 for `--new-transform-fname`) is
//!   copied past its buffer: both modes keep it whole, noted.
//! - **FS-1040** (memory): `--rescale-voxel` with an argument that is not a number leaves that
//!   index as memory contents: both modes 0, noted.
//! - **FS-1041** (memory): `-f` with more than 2,000 frames writes past its array: both modes
//!   keep them all, noted.
//! - **FS-1042** (crash): `--debug` where `debug.gdb` cannot be created: both modes write no file,
//!   noted.

use std::io::Write;
use std::path::Path;

use larmorx_core::cfmt;
use larmorx_core::cscan::strtod;
use larmorx_core::defects::{FS_1038, FS_1039, FS_1040, FS_1041, FS_1042};
use larmorx_freesurfer_core::cscan::{Bytes, Scanf};
use larmorx_freesurfer_core::cstd::is_space;
use larmorx_freesurfer_core::diag::{DIAG_INFO, diag_init};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::help_xml::output_help_xml;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::Matrix;
use larmorx_freesurfer_core::matrix_io::matrix_read_txt;
use larmorx_freesurfer_core::matrix_ops::matrix_print;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::utils::{file_exists, str_lower};
use larmorx_freesurfer_core::version::{get_all_info, handle_version_option};
use larmorx_freesurfer_io::cfile::CFile;
use larmorx_freesurfer_io::identify::{
    GDF_FILE, MRI_CORONAL_SLICE_DIRECTORY, MRI_VOLUME_TYPE_UNKNOWN, mri_get_volume_name,
    mri_identify, string_to_type,
};
use larmorx_freesurfer_io::mriio::mriio_command_line;
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_INT, MRI_RGB, MRI_SHORT, MRI_UCHAR, SAMPLE_CUBIC_BSPLINE, SAMPLE_NEAREST,
    SAMPLE_TRILINEAR, SAMPLE_VOTE, SAMPLE_WEIGHTED,
};
use larmorx_freesurfer_volume::orient::check_orientation_string;

/// `mri_convert.help.xml`, the text `usage` renders.
pub const HELP_XML: &[u8] = include_bytes!("mri_convert.help.xml");

/// `STRLEN`: the size of `main`'s name buffers.
pub const STRLEN: usize = 1024;
/// The size of `NewTransformFname`.
pub const NEW_TRANSFORM_FNAME_LEN: usize = 2000;
/// The size of `frames`.
pub const MAX_FRAMES: usize = 2000;

/// What `mri_convert` writes to and reads from: standard output and error, its environment (the
/// options set some variables) and the directory relative names start in.
pub struct Run<'a, 'b> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
    pub env: &'b mut Environment,
    pub dir: Option<&'b Path>,
}

/// `DICOMRead.h`'s globals that `mri_convert`'s options set, with their initial values.
#[derive(Clone, Debug, PartialEq)]
pub struct DicomSettings {
    pub use_dcm2niix: i32,
    pub dcm2niix_create_bids: i32,
    pub dcm2niix_outdir: Option<String>,
    pub dcm2niix_no_force_stack_same_series: i32,
    pub dcm2niix_info_dump: Option<String>,
    pub dcm2niix_dicom_flist: Option<String>,
    pub dcm2niix_only_single_file: i32,
    pub dcm2niix_opts: Option<String>,
    pub do_siemens_ascii_dump: i32,
    pub do_siemens_ascii_alt_dump: i32,
    pub use_dicom_read2: i32,
    pub sdcm_status_file: Option<String>,
    pub sdcm_list_file: Option<String>,
    pub slice_res_el_tag1: i64,
    pub slice_res_el_tag2: i64,
    pub auto_slice_res_el_tag: i32,
    /// `DICOMReadFirstDicomFile` (cleared by `--first-dicom`; the DICOM reader fills it).
    pub first_dicom_file: Vec<u8>,
}

impl Default for DicomSettings {
    fn default() -> DicomSettings {
        DicomSettings {
            use_dcm2niix: 1,
            dcm2niix_create_bids: 0,
            dcm2niix_outdir: None,
            dcm2niix_no_force_stack_same_series: 0,
            dcm2niix_info_dump: None,
            dcm2niix_dicom_flist: None,
            dcm2niix_only_single_file: 0,
            dcm2niix_opts: None,
            do_siemens_ascii_dump: 0,
            do_siemens_ascii_alt_dump: 0,
            use_dicom_read2: 1,
            sdcm_status_file: None,
            sdcm_list_file: None,
            slice_res_el_tag1: 0x88,
            slice_res_el_tag2: 0x50,
            auto_slice_res_el_tag: 0,
            first_dicom_file: Vec::new(),
        }
    }
}

/// `main`'s variables that its options and checks set, the file statics and the DICOM reader's
/// settings, with FreeSurfer's initial values. A variable FreeSurfer leaves unset until an option
/// sets it is `None` until then.
#[derive(Clone, Debug)]
pub struct Options {
    /// The command line as `getAllInfo` records it (`MRIaddCommandLine` later).
    pub cmdline: String,
    /// The arguments the option loop read (`argv` after `handleVersionOption`).
    pub argv: Vec<String>,
    pub outside_val: i32,
    pub reorder_flag: i32,
    pub reorder_vals: Option<[i32; 3]>,
    pub reorder4_flag: i32,
    pub reorder4_vals: Option<[i32; 4]>,
    pub shiftdim_flag: i32,
    pub shift_dim: Option<[i32; 3]>,
    pub invert_val: f32,
    pub in_name: String,
    pub out_name: String,
    pub conform_flag: i32,
    pub conf_keep_dc: i32,
    pub conform_width_256_flag: i32,
    pub conform_min: i32,
    pub conform_size: f32,
    pub auto_align_file: Option<String>,
    pub auto_align: Option<Matrix>,
    pub nochange_flag: i32,
    pub store_orig_ras2vox_flag: i32,
    pub parse_only_flag: i32,
    pub in_info_flag: i32,
    pub out_info_flag: i32,
    pub template_info_flag: i32,
    pub in_stats_flag: i32,
    pub out_stats_flag: i32,
    pub read_only_flag: i32,
    pub no_write_flag: i32,
    pub in_matrix_flag: i32,
    pub out_matrix_flag: i32,
    pub force_ras_good: i32,
    pub sphinx_flag: i32,
    pub transform_fname: Option<String>,
    pub transform_flag: i32,
    pub invert_transform_flag: i32,
    pub out_like_name: Option<String>,
    pub out_like_flag: i32,
    pub crop_flag: i32,
    pub crop_center: [i32; 3],
    pub crop_size: [i32; 3],
    pub devxfm_subject: Option<String>,
    pub dev_xfm: i32,
    pub upsample_flag: i32,
    pub upsample_factor: i32,
    /// `in_i_size`, `in_j_size`, `in_k_size` and their flags.
    pub in_size: [Option<f32>; 3],
    pub in_size_flag: [i32; 3],
    pub out_size: [Option<f32>; 3],
    pub out_size_flag: [i32; 3],
    /// `in_i_directions`, `in_j_directions`, `in_k_directions` and their flags.
    pub in_directions: [Option<[f32; 3]>; 3],
    pub in_direction_flag: [i32; 3],
    pub out_directions: [Option<[f32; 3]>; 3],
    pub out_direction_flag: [i32; 3],
    pub colortablefile: String,
    pub in_orientation_flag: i32,
    pub in_orientation_string: Option<String>,
    pub out_orientation_flag: i32,
    pub out_orientation_string: Option<String>,
    pub in_center_flag: i32,
    pub in_center: Option<[f32; 3]>,
    pub delta_in_center_flag: i32,
    pub delta_in_center: Option<[f32; 3]>,
    pub out_center_flag: i32,
    pub out_center: Option<[f32; 3]>,
    /// `out_n_i`, `out_n_j`, `out_n_k` and their flags.
    pub out_n: [Option<i32>; 3],
    pub out_n_flag: [i32; 3],
    pub voxel_size_flag: i32,
    pub voxel_size: Option<[f32; 3]>,
    pub downsample_flag: i32,
    pub downsampleold_flag: i32,
    pub downsample2_flag: i32,
    pub downsample_factor: Option<[f32; 3]>,
    pub in_n: [Option<i32>; 3],
    pub in_n_flag: [i32; 3],
    pub antialias_flag: i32,
    pub nthreads: i32,
    pub fwhm: f32,
    pub gstd: f32,
    pub reduce: i32,
    pub in_tr_flag: i32,
    pub in_tr: f32,
    pub in_ti_flag: i32,
    pub in_ti: f32,
    pub in_te_flag: i32,
    pub in_te: f32,
    pub in_flip_angle_flag: i32,
    pub in_flip_angle: f32,
    pub out_data_type: i32,
    pub out_data_type_string: Option<String>,
    pub rescale_factor: f32,
    pub scale_factor: f32,
    pub out_scale_factor: f32,
    pub rescale_voxel: i32,
    /// `rescale_voxel_c`, `rescale_voxel_r`, `rescale_voxel_s`.
    pub rescale_voxel_crs: [Option<i32>; 3],
    pub resample_type: Option<String>,
    pub resample_type_val: i32,
    pub force_in_type_flag: i32,
    pub forced_in_type: i32,
    pub in_type_string: String,
    pub force_out_type_flag: i32,
    pub forced_out_type: i32,
    pub out_type_string: Option<String>,
    pub force_template_type_flag: i32,
    pub forced_template_type: i32,
    pub template_type_string: Option<String>,
    pub subject_name: String,
    pub gdf_image_stem: String,
    pub reslice_like_flag: i32,
    pub reslice_like_name: Option<String>,
    pub frame_flag: i32,
    pub mid_frame_flag: i32,
    pub nframes: i32,
    pub frames: Vec<i32>,
    pub erode_seg_flag: i32,
    pub n_erode_seg: Option<i32>,
    pub dil_seg_flag: i32,
    pub n_dil_seg: Option<i32>,
    pub dil_seg_mask: String,
    pub slice_bias: i32,
    pub slice_bias_alpha: f32,
    pub subsample_flag: i32,
    /// `SubSampStart`, `SubSampDelta`, `SubSampEnd`.
    pub sub_samp: [Option<i32>; 3],
    pub in_like_flag: i32,
    pub in_like_name: Option<String>,
    pub first_dicom: i32,
    pub first_dicom_file: Option<String>,
    pub roi_flag: i32,
    pub fill_parcellation_flag: i32,
    pub zero_outlines_flag: i32,
    pub smooth_parcellation_flag: i32,
    pub smooth_parcellation_count: Option<i32>,
    pub color_file_flag: i32,
    pub color_file_name: Option<String>,
    pub translate_labels_flag: i32,
    pub no_scale_flag: i32,
    pub nthframe: i32,
    pub left_right_reverse: i32,
    pub left_right_reverse_pix: i32,
    pub left_right_mirror_flag: i32,
    pub left_right_keep_flag: i32,
    pub left_right_mirror_hemi: Option<String>,
    pub left_right_swap_label: i32,
    pub lhswap: Option<Matrix>,
    pub rhswap: Option<Matrix>,
    pub flip_cols: i32,
    pub slice_reverse: i32,
    pub ascii_flag: i32,
    pub in_stat_table_flag: i32,
    pub out_stat_table_flag: i32,
    pub zero_ge_z_offset_flag: i32,
    pub nskip: i32,
    pub ndrop: i32,
    /// The checks' results: the input's name without `@` and `#` parts, the output's format.
    pub in_name_only: Option<String>,
    pub out_volume_type: i32,
    // the file statics (`mri_convert.cpp` 57-73)
    pub debug: i32,
    pub upper_thresh: f32,
    pub upper_thresh_flag: i32,
    pub ncutends: i32,
    pub cutends_flag: i32,
    pub slice_crop_flag: i32,
    pub slice_crop_start: i32,
    pub slice_crop_stop: i32,
    pub split_frames: i32,
    pub delete_cmds: i32,
    pub new_transform_fname: String,
    pub do_new_transform_fname: i32,
    pub dicom: DicomSettings,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            cmdline: String::new(),
            argv: Vec::new(),
            outside_val: 0,
            reorder_flag: 0,
            reorder_vals: None,
            reorder4_flag: 0,
            reorder4_vals: None,
            shiftdim_flag: 0,
            shift_dim: None,
            invert_val: -1.0,
            in_name: String::new(),
            out_name: String::new(),
            conform_flag: 0,
            conf_keep_dc: 0,
            conform_width_256_flag: 0,
            conform_min: 0,
            conform_size: 1.0,
            auto_align_file: None,
            auto_align: None,
            nochange_flag: 0,
            store_orig_ras2vox_flag: 0,
            parse_only_flag: 0,
            in_info_flag: 0,
            out_info_flag: 0,
            template_info_flag: 0,
            in_stats_flag: 0,
            out_stats_flag: 0,
            read_only_flag: 0,
            no_write_flag: 0,
            in_matrix_flag: 0,
            out_matrix_flag: 0,
            force_ras_good: 0,
            sphinx_flag: 0,
            transform_fname: None,
            transform_flag: 0,
            invert_transform_flag: 0,
            out_like_name: None,
            out_like_flag: 0,
            crop_flag: 0,
            crop_center: [128; 3],
            crop_size: [256; 3],
            devxfm_subject: None,
            dev_xfm: 0,
            upsample_flag: 0,
            upsample_factor: 0,
            in_size: [None; 3],
            in_size_flag: [0; 3],
            out_size: [None; 3],
            out_size_flag: [0; 3],
            in_directions: [None; 3],
            in_direction_flag: [0; 3],
            out_directions: [None; 3],
            out_direction_flag: [0; 3],
            colortablefile: String::new(),
            in_orientation_flag: 0,
            in_orientation_string: None,
            out_orientation_flag: 0,
            out_orientation_string: None,
            in_center_flag: 0,
            in_center: None,
            delta_in_center_flag: 0,
            delta_in_center: None,
            out_center_flag: 0,
            out_center: None,
            out_n: [None; 3],
            out_n_flag: [0; 3],
            voxel_size_flag: 0,
            voxel_size: None,
            downsample_flag: 0,
            downsampleold_flag: 0,
            downsample2_flag: 0,
            downsample_factor: None,
            in_n: [None; 3],
            in_n_flag: [0; 3],
            antialias_flag: 0,
            nthreads: 1,
            fwhm: -1.0,
            gstd: -1.0,
            reduce: 0,
            in_tr_flag: 0,
            in_tr: 0.0,
            in_ti_flag: 0,
            in_ti: 0.0,
            in_te_flag: 0,
            in_te: 0.0,
            in_flip_angle_flag: 0,
            in_flip_angle: 0.0,
            out_data_type: -1,
            out_data_type_string: None,
            rescale_factor: 1.0,
            scale_factor: 1.0,
            out_scale_factor: 1.0,
            rescale_voxel: 0,
            rescale_voxel_crs: [None; 3],
            resample_type: None,
            resample_type_val: SAMPLE_TRILINEAR,
            force_in_type_flag: 0,
            forced_in_type: MRI_VOLUME_TYPE_UNKNOWN,
            in_type_string: String::new(),
            force_out_type_flag: 0,
            forced_out_type: MRI_VOLUME_TYPE_UNKNOWN,
            out_type_string: None,
            force_template_type_flag: 0,
            forced_template_type: MRI_VOLUME_TYPE_UNKNOWN,
            template_type_string: None,
            subject_name: String::new(),
            gdf_image_stem: String::new(),
            reslice_like_flag: 0,
            reslice_like_name: None,
            frame_flag: 0,
            mid_frame_flag: 0,
            nframes: 0,
            frames: Vec::new(),
            erode_seg_flag: 0,
            n_erode_seg: None,
            dil_seg_flag: 0,
            n_dil_seg: None,
            dil_seg_mask: String::new(),
            slice_bias: 0,
            slice_bias_alpha: 1.0,
            subsample_flag: 0,
            sub_samp: [None; 3],
            in_like_flag: 0,
            in_like_name: None,
            first_dicom: 0,
            first_dicom_file: None,
            roi_flag: 0,
            fill_parcellation_flag: 0,
            zero_outlines_flag: 0,
            smooth_parcellation_flag: 0,
            smooth_parcellation_count: None,
            color_file_flag: 0,
            color_file_name: None,
            translate_labels_flag: 1,
            no_scale_flag: 0,
            nthframe: -1,
            left_right_reverse: 0,
            left_right_reverse_pix: 0,
            left_right_mirror_flag: 0,
            left_right_keep_flag: 0,
            left_right_mirror_hemi: None,
            left_right_swap_label: 0,
            lhswap: None,
            rhswap: None,
            flip_cols: 0,
            slice_reverse: 0,
            ascii_flag: 0,
            in_stat_table_flag: 0,
            out_stat_table_flag: 0,
            zero_ge_z_offset_flag: 0,
            nskip: 0,
            ndrop: 0,
            in_name_only: None,
            out_volume_type: MRI_VOLUME_TYPE_UNKNOWN,
            debug: 0,
            upper_thresh: 0.0,
            upper_thresh_flag: 0,
            ncutends: 0,
            cutends_flag: 0,
            slice_crop_flag: 0,
            slice_crop_start: 0,
            slice_crop_stop: 0,
            split_frames: 0,
            delete_cmds: 0,
            new_transform_fname: String::new(),
            do_new_transform_fname: 0,
            dicom: DicomSettings::default(),
        }
    }
}

/// `usage_message(stdout)`: `\ntype <progname> -u for usage\n\n`.
pub fn usage_message(g: &Globals, out: &mut dyn Write) {
    let _ = write!(out, "\ntype {} -u for usage\n\n", g.progname);
}

/// `usage(stdout)`: the help (`outputHelpXml` of [`HELP_XML`]).
pub fn usage(g: &mut Globals, r: &mut Run) {
    output_help_xml(g, r.out, r.err, Some(HELP_XML));
}

/// An error on standard error, the usage hint on standard output, status 1.
fn fail_usage(g: &Globals, r: &mut Run, message: &str) -> Fatal {
    let _ = r.err.write_all(message.as_bytes());
    usage_message(g, r.out);
    Fatal::Exit(1)
}

/// An error on standard error, status 1.
fn fail_err(r: &mut Run, message: &str) -> Fatal {
    let _ = r.err.write_all(message.as_bytes());
    Fatal::Exit(1)
}

/// An error on standard output, status 1.
fn fail_out(r: &mut Run, message: &str) -> Fatal {
    let _ = r.out.write_all(message.as_bytes());
    Fatal::Exit(1)
}

/// `strtol(s, &end, 10)`: the value (beyond the `long` range `LONG_MAX` or `LONG_MIN`) and the
/// bytes read (0 when there is no number: the end is the string's start).
pub fn strtol(s: &[u8]) -> (i64, usize) {
    let mut i = 0;
    while i < s.len() && is_space(s[i]) {
        i += 1;
    }
    let neg = s.get(i) == Some(&b'-');
    if matches!(s.get(i), Some(b'-' | b'+')) {
        i += 1;
    }
    let start = i;
    let mut v: i128 = 0;
    while i < s.len() && s[i].is_ascii_digit() {
        v = (v * 10 + i128::from(s[i] - b'0')).min(i128::from(i64::MAX) + 1);
        i += 1;
    }
    if i == start {
        return (0, 0);
    }
    let v = if neg { -v } else { v };
    (v.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64, i)
}

/// `sscanf(s, "%d", &v)`: the value, or `None` when it assigns none (the variable left as it was).
fn sscanf_d(g: &mut Globals, s: &str) -> Option<i32> {
    let mut b = Bytes::new(s.as_bytes());
    let mut sc = Scanf::new(&mut b);
    let v = sc.d(&mut g.errno, true);
    if sc.finish(&mut g.errno) == 1 { v } else { None }
}

/// The C program's `%g` of a `float`.
fn gf(v: f32) -> String {
    cfmt::g(f64::from(v), 6)
}

/// The argument list and the option loop's position.
struct Args<'s> {
    a: &'s [String],
}

impl Args<'_> {
    /// `get_ints(argc, argv, &i, vals, n)`.
    fn ints(&self, g: &Globals, r: &mut Run, i: &mut usize, n: usize) -> Result<Vec<i32>, Fatal> {
        let a = self.a;
        let opt = &a[*i];
        if *i + n >= a.len() {
            let after = a.len() as i64 - *i as i64 - 1;
            return Err(fail_usage(
                g,
                r,
                &format!(
                    "\n{}: argument {opt} expects {n} integers; only {after} arguments after flag\n",
                    g.progname
                ),
            ));
        }
        let mut vals = Vec::with_capacity(n);
        for k in 0..n {
            let s = &a[*i + k + 1];
            if s.is_empty() {
                return Err(fail_usage(
                    g,
                    r,
                    &format!("\n{}: argument to {opt} flag is null\n", g.progname),
                ));
            }
            let (v, end) = strtol(s.as_bytes());
            vals.push(v as i32);
            if end != s.len() {
                return Err(fail_usage(
                    g,
                    r,
                    &format!(
                        "\n{}: error converting \"{s}\" to an integer for {opt} flag, incorrect # of args (need {n})?\n",
                        g.progname
                    ),
                ));
            }
        }
        *i += n;
        Ok(vals)
    }

    fn int(&self, g: &Globals, r: &mut Run, i: &mut usize) -> Result<i32, Fatal> {
        Ok(self.ints(g, r, i, 1)?[0])
    }

    /// `get_floats(argc, argv, &i, vals, n)`: `(float) strtod(...)`.
    fn floats(
        &self,
        g: &Globals,
        r: &mut Run,
        i: &mut usize,
        n: usize,
    ) -> Result<Vec<f32>, Fatal> {
        let a = self.a;
        let opt = &a[*i];
        if *i + n >= a.len() {
            let after = a.len() as i64 - *i as i64 - 1;
            return Err(fail_usage(
                g,
                r,
                &format!(
                    "\n{}: argument {opt} expects {n} floats; only {after} arguments after flag\n",
                    g.progname
                ),
            ));
        }
        let mut vals = Vec::with_capacity(n);
        for k in 0..n {
            let s = &a[*i + k + 1];
            if s.is_empty() {
                return Err(fail_usage(
                    g,
                    r,
                    &format!("\n{}: argument to {opt} flag is null\n", g.progname),
                ));
            }
            let (v, end) = strtod(s.as_bytes());
            vals.push(v as f32);
            if end != s.len() {
                return Err(fail_usage(
                    g,
                    r,
                    &format!(
                        "\n{}: error converting \"{s}\" to a float for {opt} flag: incorrect # of args? Need {n}\n",
                        g.progname
                    ),
                ));
            }
        }
        *i += n;
        Ok(vals)
    }

    fn float(&self, g: &Globals, r: &mut Run, i: &mut usize) -> Result<f32, Fatal> {
        Ok(self.floats(g, r, i, 1)?[0])
    }

    fn three(&self, g: &Globals, r: &mut Run, i: &mut usize) -> Result<[f32; 3], Fatal> {
        let v = self.floats(g, r, i, 3)?;
        Ok([v[0], v[1], v[2]])
    }

    /// `get_string(argc, argv, &i, val)` into a buffer of `cap` bytes (FS-1039 past it).
    fn string(
        &self,
        g: &mut Globals,
        r: &mut Run,
        i: &mut usize,
        cap: usize,
    ) -> Result<String, Fatal> {
        let a = self.a;
        if *i + 1 >= a.len() {
            return Err(fail_usage(
                g,
                r,
                &format!(
                    "\n{}: argument {} expects an extra argument; none found\n",
                    g.progname, a[*i]
                ),
            ));
        }
        let s = a[*i + 1].clone();
        fits(g, &a[*i], &s, cap);
        *i += 1;
        Ok(s)
    }
}

/// FS-1039: a name copied into a buffer of `cap` bytes.
fn fits(g: &mut Globals, what: &str, s: &str, cap: usize) {
    if s.len() >= cap {
        g.notices.add(
            &FS_1039,
            format!(
                "{what}: a name of {} bytes, longer than FreeSurfer's {cap}-byte buffer (it writes past it); kept whole",
                s.len()
            ),
        );
    }
}

/// `--in_i_direction` and its kin: three floats, normalised when their length is not 1 (in
/// `float`, the square root of the sum of the squares in `double`); a zero length ends the
/// program.
fn direction(
    g: &Globals,
    r: &mut Run,
    args: &Args,
    i: &mut usize,
    name: &str,
    stderr_name: &str,
) -> Result<[f32; 3], Fatal> {
    let mut d = args.three(g, r, i)?;
    let magnitude = (f64::from(d[0] * d[0] + d[1] * d[1] + d[2] * d[2])).sqrt() as f32;
    if magnitude == 0.0 {
        return Err(fail_usage(
            g,
            r,
            &format!(
                "\n{}: directions must have non-zero magnitude; {stderr_name} = ({}, {}, {})\n",
                g.progname,
                gf(d[0]),
                gf(d[1]),
                gf(d[2])
            ),
        ));
    }
    if magnitude != 1.0 {
        let _ = write!(
            r.out,
            "normalizing {name}: ({}, {}, {}) -> ",
            gf(d[0]),
            gf(d[1]),
            gf(d[2])
        );
        for x in &mut d {
            *x /= magnitude;
        }
        let _ = writeln!(r.out, "({}, {}, {})", gf(d[0]), gf(d[1]), gf(d[2]));
    }
    Ok(d)
}

/// `MatrixReadTxt(fname, NULL)`.
fn read_matrix(g: &mut Globals, r: &mut Run, fname: &str) -> Result<Option<Matrix>, Fatal> {
    Ok(matrix_read_txt(g, r.out, r.err, r.dir, fname)?.into_new())
}

/// `--debug` before everything else: `debug.gdb`, the command line for gdb (FS-1042 where it
/// cannot be created).
fn write_debug_gdb(g: &mut Globals, r: &mut Run, argv: &[String]) {
    let Some(_) = argv.iter().position(|a| a == "--debug") else {
        return;
    };
    let mut fp = match CFile::open(r.dir, "debug.gdb", "w") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            g.notices.add(
                &FS_1042,
                "--debug: debug.gdb cannot be created (FreeSurfer writes to a NULL stream and crashes); no file",
            );
            return;
        }
    };
    let mut text = format!(
        "# source this file in gdb to debug\nfile {} \nrun ",
        argv[0]
    );
    for a in argv.iter().skip(1).filter(|a| *a != "--debug") {
        text.push_str(a);
        text.push(' ');
    }
    text.push('\n');
    fp.put_bytes(text.as_bytes());
    fp.close();
}

/// `atof(argv[i + 1])` of `--rescale`, `--scale` and `--out-scale` (FS-1038 without one).
fn atof_next(g: &mut Globals, r: &mut Run, a: &[String], i: &mut usize) -> Result<f32, Fatal> {
    let Some(s) = a.get(*i + 1) else {
        g.notices.add(
            &FS_1038,
            format!(
                "{}: no argument after it (FreeSurfer converts a NULL string and crashes)",
                a[*i]
            ),
        );
        return Err(fail_usage(
            g,
            r,
            &format!(
                "\n{}: argument {} expects 1 floats; only 0 arguments after flag\n",
                g.progname, a[*i]
            ),
        ));
    };
    let v = strtod(s.as_bytes()).0 as f32;
    *i += 1;
    Ok(v)
}

/// `main` from its start to the end of its option loop and the checks that follow it
/// (`mri_convert.cpp` 233-1676): the help when there are no arguments (status 1), the version
/// options, then each option.
pub fn parse_options(g: &mut Globals, r: &mut Run, argv: &[String]) -> Result<Options, Fatal> {
    diag_init(g, r.env, r.out);
    let mut o = Options {
        cmdline: get_all_info(g, r.env, argv, "mri_convert"),
        ..Options::default()
    };
    for a in argv {
        let _ = write!(r.out, "{a} ");
    }
    let _ = writeln!(r.out);
    let _ = r.out.flush();
    write_debug_gdb(g, r, argv);
    g.progname = match argv[0].rfind('/') {
        Some(n) => argv[0][n + 1..].to_owned(),
        None => argv[0].clone(),
    };
    mriio_command_line(argv);
    if argv.len() == 1 {
        usage(g, r);
        return Err(Fatal::Exit(1));
    }
    let (nargs, left) = handle_version_option(g, r.env, r.out, argv, "mri_convert");
    if nargs != 0 && left.len() == 1 {
        return Err(Fatal::Exit(0));
    }
    o.argv = left;
    let a: Vec<String> = o.argv.clone();
    let args = Args { a: &a };
    let argc = a.len();
    let mut i = 1;
    while i < argc {
        let arg = a[i].as_str();
        match arg {
            "-version2" => return Err(Fatal::Exit(97)),
            "-r" | "--reorder" => {
                let v = args.ints(g, r, &mut i, 3)?;
                o.reorder_vals = Some([v[0], v[1], v[2]]);
                o.reorder_flag = 1;
            }
            "-r4" | "--reorder4" => {
                let v = args.ints(g, r, &mut i, 4)?;
                o.reorder4_vals = Some([v[0], v[1], v[2], v[3]]);
                o.reorder4_flag = 1;
            }
            "--shift" => {
                let v = args.ints(g, r, &mut i, 3)?;
                o.shift_dim = Some([v[0], v[1], v[2]]);
                o.shiftdim_flag = 1;
            }
            "-ut" | "--upper_thresh" => {
                o.upper_thresh = args.float(g, r, &mut i)?;
                o.upper_thresh_flag = 1;
            }
            "-oval" | "--outside_val" => {
                o.outside_val = args.int(g, r, &mut i)?;
                let _ = writeln!(r.out, "setting outside val to {}", o.outside_val);
            }
            "--no-dwi" => r.env.set("FS_LOAD_DWI", "0"),
            "--debug" => o.debug = 1,
            "--left-right-reverse" => o.left_right_reverse = 1,
            "--left-right-reverse-pix" => o.left_right_reverse_pix = 1,
            "--left-right-mirror" | "--left-right-keep" => {
                let mirror = arg == "--left-right-mirror";
                if mirror {
                    o.left_right_mirror_flag = 1;
                } else {
                    o.left_right_keep_flag = 1;
                }
                let h = args.string(g, r, &mut i, STRLEN)?;
                o.left_right_mirror_hemi = Some(h.clone());
                if h != "lh" && h != "rh" {
                    return Err(fail_out(
                        r,
                        &format!("ERROR: pass either 'lh' or 'rh' with {arg}!\n"),
                    ));
                }
            }
            "--left-right-swap-label" => o.left_right_swap_label = 1,
            "--left-right-swap-label-table" => {
                let lh = args.string(g, r, &mut i, STRLEN)?;
                o.lhswap = read_matrix(g, r, &lh)?;
                let rh = args.string(g, r, &mut i, STRLEN)?;
                o.rhswap = read_matrix(g, r, &rh)?;
                o.left_right_swap_label = 1;
            }
            "--flip-cols" => o.flip_cols = 1,
            "--slice-reverse" => o.slice_reverse = 1,
            "--ascii" | "--ascii+crsf" | "--ascii-fcol" => {
                o.ascii_flag = match arg {
                    "--ascii" => 1,
                    "--ascii+crsf" => 2,
                    _ => 3,
                };
                o.force_out_type_flag = 1;
            }
            "--in_stats_table" => o.in_stat_table_flag = 1,
            "--out_stats_table" => o.out_stat_table_flag = 1,
            "--invert_contrast" => o.invert_val = args.float(g, r, &mut i)?,
            "-i" | "--input_volume" => o.in_name = args.string(g, r, &mut i, STRLEN)?,
            "-o" | "--output_volume" => o.out_name = args.string(g, r, &mut i, STRLEN)?,
            "-c" | "--conform" => o.conform_flag = 1,
            "--conform-dc" => {
                o.conform_flag = 1;
                o.conf_keep_dc = 1;
            }
            "--cw256" => {
                o.conform_flag = 1;
                o.conform_width_256_flag = 1;
            }
            "--delete-cmds" => o.delete_cmds = 1,
            "--new-transform-fname" => {
                o.new_transform_fname = args.string(g, r, &mut i, NEW_TRANSFORM_FNAME_LEN)?;
                o.do_new_transform_fname = 1;
            }
            "--sphinx" => o.sphinx_flag = 1,
            "--no-rescale-dicom" => r.env.set("FS_RESCALE_DICOM", "0"),
            "--bvec-scanner" => r.env.set("FS_DESIRED_BVEC_SPACE", "1"),
            "--bvec-voxel" => r.env.set("FS_DESIRED_BVEC_SPACE", "2"),
            "--no-analyze-rescale" => r.env.set("FS_ANALYZE_NO_RESCALE", "1"),
            "--autoalign" => {
                let f = args.string(g, r, &mut i, STRLEN)?;
                o.auto_align = read_matrix(g, r, &f)?;
                o.auto_align_file = Some(f);
                let _ = writeln!(r.out, "Auto Align Matrix");
                let out: &mut dyn Write = &mut *r.out;
                let mut sink = Vec::new();
                matrix_print(g, Some(out), &mut sink, r.err, o.auto_align.as_ref(), 5);
                let _ = r.out.write_all(&sink);
            }
            "-nc" | "--nochange" => o.nochange_flag = 1,
            "-so" | "--store_orig_ras2vox" => o.store_orig_ras2vox_flag = 1,
            "-cm" | "--conform_min" => {
                o.conform_min = 1;
                o.conform_flag = 1;
            }
            "-cs" | "--conform_size" => {
                o.conform_size = args.float(g, r, &mut i)?;
                o.conform_flag = 1;
            }
            "-po" | "--parse_only" => o.parse_only_flag = 1,
            "-ii" | "--in_info" => o.in_info_flag = 1,
            "-oi" | "--out_info" => o.out_info_flag = 1,
            "-ti" | "--template_info" => o.template_info_flag = 1,
            "-is" | "--in_stats" => o.in_stats_flag = 1,
            "-os" | "--out_stats" => o.out_stats_flag = 1,
            "-ro" | "--read_only" => o.read_only_flag = 1,
            "-nw" | "--no_write" => o.no_write_flag = 1,
            "-im" | "--in_matrix" => o.in_matrix_flag = 1,
            "-om" | "--out_matrix" => o.out_matrix_flag = 1,
            "--force_ras_good" => o.force_ras_good = 1,
            "--split" => o.split_frames = 1,
            "-at" | "--apply_transform" | "-T" => {
                o.transform_fname = Some(args.string(g, r, &mut i, STRLEN)?);
                o.transform_flag = 1;
                o.invert_transform_flag = 0;
            }
            "--like" => {
                o.out_like_name = Some(args.string(g, r, &mut i, STRLEN)?);
                o.out_like_flag = 1;
            }
            "--crop" => {
                o.crop_flag = 1;
                let v = args.ints(g, r, &mut i, 3)?;
                o.crop_center = [v[0], v[1], v[2]];
            }
            "--slice-crop" => {
                o.slice_crop_flag = 1;
                o.slice_crop_start = args.int(g, r, &mut i)?;
                o.slice_crop_stop = args.int(g, r, &mut i)?;
                if o.slice_crop_start > o.slice_crop_stop {
                    return Err(fail_err(r, "ERROR: s_start > s_end\n"));
                }
            }
            "--cropsize" => {
                o.crop_flag = 1;
                let v = args.ints(g, r, &mut i, 3)?;
                o.crop_size = [v[0], v[1], v[2]];
            }
            "--devolvexfm" => {
                o.devxfm_subject = Some(args.string(g, r, &mut i, STRLEN)?);
                o.dev_xfm = 1;
            }
            "-ait" | "--apply_inverse_transform" => {
                let t = args.string(g, r, &mut i, STRLEN)?;
                o.transform_fname = Some(t.clone());
                if !file_exists(r.dir, &t) {
                    return Err(fail_err(
                        r,
                        &format!("ERROR: cannot find transform file {t}\n"),
                    ));
                }
                o.transform_flag = 1;
                o.invert_transform_flag = 1;
            }
            "--upsample" => {
                o.upsample_factor = args.int(g, r, &mut i)?;
                o.upsample_flag = 1;
            }
            "-iis" | "--in_i_size" | "-ijs" | "--in_j_size" | "-iks" | "--in_k_size" => {
                let k = axis(arg);
                o.in_size[k] = Some(args.float(g, r, &mut i)?);
                o.in_size_flag[k] = 1;
            }
            "-ois" | "--out_i_size" | "-ojs" | "--out_j_size" | "-oks" | "--out_k_size" => {
                let k = axis(arg);
                o.out_size[k] = Some(args.float(g, r, &mut i)?);
                o.out_size_flag[k] = 1;
            }
            "-iid" | "--in_i_direction" | "-ijd" | "--in_j_direction" | "-ikd"
            | "--in_k_direction" => {
                let k = axis(arg);
                let name = ["in_i_direction", "in_j_direction", "in_k_direction"][k];
                o.in_directions[k] = Some(direction(g, r, &args, &mut i, name, name)?);
                o.in_direction_flag[k] = 1;
            }
            "-oid" | "--out_i_direction" | "-ojd" | "--out_j_direction" | "-okd"
            | "--out_k_direction" => {
                let k = axis(arg);
                let name = ["out_i_direction", "out_j_direction", "out_k_direction"][k];
                o.out_directions[k] = Some(direction(g, r, &args, &mut i, name, name)?);
                o.out_direction_flag[k] = 1;
            }
            "--ctab" => o.colortablefile = args.string(g, r, &mut i, STRLEN)?,
            "--no-ctab" | "--remove-ctab" => o.colortablefile = "remove".to_owned(),
            "-io" | "--in_orientation" | "-oo" | "--out_orientation" => {
                let input = arg == "-io" || arg == "--in_orientation";
                let s = args.string(g, r, &mut i, STRLEN)?;
                if input {
                    o.in_orientation_string = Some(s.clone());
                } else {
                    o.out_orientation_string = Some(s.clone());
                }
                if let Some(msg) = check_orientation_string(g, s.as_bytes()) {
                    let what = if input {
                        "in orientation"
                    } else {
                        "out_orientation"
                    };
                    let _ = write!(r.out, "ERROR: with {what} string {s}\n");
                    let _ = r.out.write_all(&msg);
                    let _ = r.out.write_all(b"\n");
                    return Err(Fatal::Exit(1));
                }
                if input {
                    o.in_orientation_flag = 1;
                } else {
                    o.out_orientation_flag = 1;
                }
            }
            "-ic" | "--in_center" => {
                o.in_center = Some(args.three(g, r, &mut i)?);
                o.in_center_flag = 1;
            }
            "-dic" | "--delta_in_center" => {
                o.delta_in_center = Some(args.three(g, r, &mut i)?);
                o.delta_in_center_flag = 1;
            }
            "-oc" | "--out_center" => {
                o.out_center = Some(args.three(g, r, &mut i)?);
                o.out_center_flag = 1;
            }
            "-oni" | "-oic" | "--out_i_count" | "-onj" | "-ojc" | "--out_j_count" | "-onk"
            | "-okc" | "--out_k_count" => {
                let k = axis(arg);
                o.out_n[k] = Some(args.int(g, r, &mut i)?);
                o.out_n_flag[k] = 1;
            }
            "-vs" | "--voxsize" | "-voxsize" => {
                o.voxel_size = Some(args.three(g, r, &mut i)?);
                o.voxel_size_flag = 1;
            }
            "-ds" | "--downsample" | "-downsample" => {
                o.downsample_factor = Some(args.three(g, r, &mut i)?);
                o.downsample_flag = 1;
            }
            "-dsold" | "--downsampleold" | "-downsampleold" => {
                o.downsample_factor = Some(args.three(g, r, &mut i)?);
                o.downsampleold_flag = 1;
            }
            "-ds2" | "--downsample2" => o.downsample2_flag = 1,
            "-ini" | "-iic" | "--in_i_count" | "-inj" | "-ijc" | "--in_j_count" | "-ink"
            | "-ikc" | "--in_k_count" => {
                let k = axis(arg);
                o.in_n[k] = Some(args.int(g, r, &mut i)?);
                o.in_n_flag[k] = 1;
            }
            "--antialias" => o.antialias_flag = 1,
            "--nthreads" | "--threads" => {
                o.nthreads = args.int(g, r, &mut i)?;
                let _ = write!(r.out, "\nSet OPEN MP NUM threads to {}\n", o.nthreads);
            }
            "--fwhm" => {
                o.fwhm = args.float(g, r, &mut i)?;
                o.gstd = (f64::from(o.fwhm) / sqrt_log_256()) as f32;
                let _ = writeln!(r.out, "fwhm = {}, gstd = {}", gf(o.fwhm), gf(o.gstd));
            }
            "--reduce" => {
                o.reduce = args.int(g, r, &mut i)?;
                let _ = writeln!(r.out, "reducing input image {} times", o.reduce);
            }
            "-tr" => {
                o.in_tr = args.float(g, r, &mut i)?;
                o.in_tr_flag = 1;
            }
            "-TI" => {
                o.in_ti = args.float(g, r, &mut i)?;
                o.in_ti_flag = 1;
            }
            "-te" => {
                o.in_te = args.float(g, r, &mut i)?;
                o.in_te_flag = 1;
            }
            "-flip_angle" => {
                o.in_flip_angle = args.float(g, r, &mut i)?;
                o.in_flip_angle_flag = 1;
            }
            "-odt" | "--out_data_type" => {
                let s = str_lower(&args.string(g, r, &mut i, STRLEN)?);
                o.out_data_type_string = Some(s.clone());
                o.out_data_type = match s.as_str() {
                    "uchar" => MRI_UCHAR,
                    "short" => MRI_SHORT,
                    "int" => MRI_INT,
                    "float" => MRI_FLOAT,
                    "rgb" => MRI_RGB,
                    _ => {
                        return Err(fail_usage(
                            g,
                            r,
                            &format!("\n{}: unknown data type \"{}\"\n", g.progname, a[i]),
                        ));
                    }
                };
            }
            "--bfile-little-endian" => r.env.set("BFILE_LITTLE_ENDIAN", "1"),
            "--rescale" => o.rescale_factor = atof_next(g, r, &a, &mut i)?,
            "-sc" | "--scale" => o.scale_factor = atof_next(g, r, &a, &mut i)?,
            "-osc" | "--out-scale" => o.out_scale_factor = atof_next(g, r, &a, &mut i)?,
            "--rescale-voxel" => {
                o.rescale_voxel = 1;
                if i + 3 >= argc {
                    g.notices.add(
                        &FS_1038,
                        format!(
                            "--rescale-voxel: {} of its 3 arguments (FreeSurfer reads a NULL string and crashes)",
                            argc - 1 - i
                        ),
                    );
                    return Err(fail_usage(
                        g,
                        r,
                        &format!(
                            "\n{}: argument --rescale-voxel expects 3 integers; only {} arguments after flag\n",
                            g.progname,
                            argc - 1 - i
                        ),
                    ));
                }
                for k in 0..3 {
                    let v = sscanf_d(g, &a[i + 1]);
                    if v.is_none() {
                        g.notices.add(
                            &FS_1040,
                            format!(
                                "--rescale-voxel {}: not a number (FreeSurfer leaves the index as memory contents); 0",
                                a[i + 1]
                            ),
                        );
                    }
                    o.rescale_voxel_crs[k] = Some(v.unwrap_or(0));
                    i += 1;
                }
                let [c, rr, s] = o.rescale_voxel_crs.map(|v| v.unwrap_or(0));
                let _ = writeln!(r.out, "rescale voxel {c} {rr} {s}");
            }
            "-rt" | "--resample_type" => {
                let s = str_lower(&args.string(g, r, &mut i, STRLEN)?);
                o.resample_type = Some(s.clone());
                o.resample_type_val = match s.as_str() {
                    "interpolate" => SAMPLE_TRILINEAR,
                    "nearest" => SAMPLE_NEAREST,
                    "vote" => SAMPLE_VOTE,
                    "weighted" => SAMPLE_WEIGHTED,
                    "cubic" => SAMPLE_CUBIC_BSPLINE,
                    _ => {
                        return Err(fail_usage(
                            g,
                            r,
                            &format!("\n{}: unknown resample type \"{}\"\n", g.progname, a[i]),
                        ));
                    }
                };
            }
            "-it" | "--in_type" => {
                o.in_type_string = args.string(g, r, &mut i, STRLEN)?;
                o.forced_in_type = string_to_type(g, &o.in_type_string);
                o.force_in_type_flag = 1;
            }
            "-dicomread3" | "-dcm2niix" | "--dcm2niix" => o.dicom.use_dcm2niix = 1,
            "-no-dcm2niix" | "--no-dcm2niix" => o.dicom.use_dcm2niix = 0,
            "-createBIDS" | "--createBIDS" | "-dcm2niix-createBIDS" | "--dcm2niix-createBIDS" => {
                o.dicom.dcm2niix_create_bids = 1;
            }
            "-dcm2niix-outdir" | "--dcm2niix-outdir" => {
                i += 1;
                o.dicom.dcm2niix_outdir = a.get(i).cloned();
            }
            "-dcm2niix-no-ForceStackSameSeries" | "--dcm2niix-no-ForceStackSameSeries" => {
                o.dicom.dcm2niix_no_force_stack_same_series = 1;
            }
            "-dcm2niix-info-dump" | "--dcm2niix-info-dump" => {
                i += 1;
                o.dicom.dcm2niix_info_dump = a.get(i).cloned();
            }
            "-dcm2niix-dicom-flist" | "--dcm2niix-dicom-flist" => {
                i += 1;
                o.dicom.dcm2niix_dicom_flist = a.get(i).cloned();
            }
            "-dcm2niix-singlefile" | "--dcm2niix-singlefile" => {
                o.dicom.dcm2niix_only_single_file = 1;
            }
            "-dcm2niix-opts" | "--dcm2niix-opts" => {
                if i + 1 >= argc {
                    g.notices.add(
                        &FS_1038,
                        format!(
                            "{arg}: no argument after it (FreeSurfer measures a NULL string and crashes)"
                        ),
                    );
                    return Err(fail_usage(
                        g,
                        r,
                        &format!(
                            "\n{}: argument {arg} expects an extra argument; none found\n",
                            g.progname
                        ),
                    ));
                }
                i += 1;
                o.dicom.dcm2niix_opts = Some(a[i].clone());
            }
            "-siemens-ascii-dump" => o.dicom.do_siemens_ascii_dump = 1,
            "-siemens-ascii-alt-dump" => o.dicom.do_siemens_ascii_alt_dump = 1,
            "-dicomread2" => o.dicom.use_dicom_read2 = 1,
            "-dicomread0" => o.dicom.use_dicom_read2 = 0,
            "-siemensBVecsCross" => r.env.set("FS_dcmGetDWIParamsSiemens_VoxelSpace", "1"),
            "-no-siemensBVecsCross" => r.env.remove("FS_dcmGetDWIParamsSiemens_VoxelSpace"),
            "-ot" | "--out_type" => {
                let s = args.string(g, r, &mut i, STRLEN)?;
                o.forced_out_type = string_to_type(g, &s);
                o.out_type_string = Some(s);
                o.force_out_type_flag = 1;
            }
            "-tt" | "--template_type" => {
                let s = args.string(g, r, &mut i, STRLEN)?;
                o.forced_template_type = string_to_type(g, &s);
                o.template_type_string = Some(s);
                o.force_template_type_flag = 1;
            }
            "-sn" | "--subject_name" => o.subject_name = args.string(g, r, &mut i, STRLEN)?,
            "-gis" | "--gdf_image_stem" => {
                o.gdf_image_stem = args.string(g, r, &mut i, STRLEN)?;
                if o.gdf_image_stem.is_empty() {
                    return Err(fail_usage(
                        g,
                        r,
                        &format!("\n{}: zero length GDF image stem given\n", g.progname),
                    ));
                }
            }
            "-rl" | "--reslice_like" => {
                o.reslice_like_name = Some(args.string(g, r, &mut i, STRLEN)?);
                o.reslice_like_flag = 1;
            }
            "-f" | "--frame" => {
                // the frame numbers: argv[i + 1] and each following argument that does not start
                // with `-` and is an integer (`strtol` reads it whole)
                let mut n = 0;
                loop {
                    n += 1;
                    let Some(next) = a.get(i + 1 + n) else {
                        break;
                    };
                    if next.starts_with('-') {
                        break;
                    }
                    if strtol(next.as_bytes()).1 != next.len() {
                        break;
                    }
                }
                if n > MAX_FRAMES {
                    g.notices.add(
                        &FS_1041,
                        format!(
                            "-f: {n} frames, more than FreeSurfer's array of {MAX_FRAMES} (it writes past it); kept"
                        ),
                    );
                }
                o.nframes = n as i32;
                o.frames = args.ints(g, r, &mut i, n)?;
                o.frame_flag = 1;
            }
            "--erode-seg" => {
                o.n_erode_seg = Some(args.int(g, r, &mut i)?);
                o.erode_seg_flag = 1;
            }
            "--dil-seg" => {
                o.n_dil_seg = Some(args.int(g, r, &mut i)?);
                o.dil_seg_flag = 1;
            }
            "--dil-seg-mask" => o.dil_seg_mask = args.string(g, r, &mut i, STRLEN)?,
            "--cutends" => {
                o.ncutends = args.int(g, r, &mut i)?;
                o.cutends_flag = 1;
            }
            "--slice-bias" => {
                o.slice_bias_alpha = args.float(g, r, &mut i)?;
                o.slice_bias = 1;
            }
            "--mid-frame" => {
                o.frame_flag = 1;
                o.mid_frame_flag = 1;
            }
            "--fsubsample" => {
                for k in 0..3 {
                    o.sub_samp[k] = Some(args.int(g, r, &mut i)?);
                }
                if o.sub_samp[1] == Some(0) {
                    return Err(fail_out(r, "ERROR: don't use subsample delta = 0\n"));
                }
                o.subsample_flag = 1;
            }
            "-il" | "--in_like" => {
                o.in_like_name = Some(args.string(g, r, &mut i, STRLEN)?);
                o.in_like_flag = 1;
            }
            "--first-dicom" => {
                o.first_dicom_file = Some(args.string(g, r, &mut i, STRLEN)?);
                o.first_dicom = 1;
                o.dicom.first_dicom_file.clear();
            }
            "-roi" | "--roi" => o.roi_flag = 1,
            "-fp" | "--fill_parcellation" => o.fill_parcellation_flag = 1,
            "-zo" | "--zero_outlines" => o.zero_outlines_flag = 1,
            "-sp" | "--smooth_parcellation" => {
                let n = args.int(g, r, &mut i)?;
                o.smooth_parcellation_count = Some(n);
                if !(14..=26).contains(&n) {
                    return Err(fail_usage(
                        g,
                        r,
                        &format!(
                            "\n{}: clean parcellation count must be between 14 and 26, inclusive\n",
                            g.progname
                        ),
                    ));
                }
                o.smooth_parcellation_flag = 1;
            }
            "-cf" | "--color_file" => {
                o.color_file_name = Some(args.string(g, r, &mut i, STRLEN)?);
                o.color_file_flag = 1;
            }
            "-nt" | "--no_translate" => o.translate_labels_flag = 0,
            "-ns" | "--no_scale" => o.no_scale_flag = args.int(g, r, &mut i)?,
            "-nth" | "--nth_frame" => o.nthframe = args.int(g, r, &mut i)?,
            "-cg" | "--crop_gdf" => g.gdf_crop_flag = true,
            "-u" | "--usage" | "--help" | "-h" => {
                usage(g, r);
                return Err(Fatal::Exit(0));
            }
            "--status" | "--statusfile" => {
                if argc - 1 - i < 1 {
                    return Err(fail_err(
                        r,
                        "ERROR: option --statusfile requires one argument\n",
                    ));
                }
                i += 1;
                let f = a[i].clone();
                o.dicom.sdcm_status_file = Some(f.clone());
                match CFile::open(r.dir, &f, "w") {
                    Ok(mut fp) => {
                        fp.put_bytes(b"0\n");
                        fp.close();
                    }
                    Err(e) => {
                        g.errno = e;
                        return Err(fail_err(
                            r,
                            &format!("ERROR: could not open {f} for writing\n"),
                        ));
                    }
                }
            }
            "--sdcmlist" => {
                if argc - 1 - i < 1 {
                    return Err(fail_err(
                        r,
                        "ERROR: option --sdcmlist requires one argument\n",
                    ));
                }
                i += 1;
                let f = a[i].clone();
                o.dicom.sdcm_list_file = Some(f.clone());
                match CFile::open(r.dir, &f, "r") {
                    Ok(fp) => {
                        fp.close();
                    }
                    Err(e) => {
                        g.errno = e;
                        return Err(fail_err(
                            r,
                            &format!("ERROR: could not open {f} for reading\n"),
                        ));
                    }
                }
            }
            "--nslices-override" | "--ncols-override" | "--nrows-override" => {
                let n = args.int(g, r, &mut i)?;
                let (label, var) = match arg {
                    "--nslices-override" => ("NSlicesOverride", "NSLICES_OVERRIDE"),
                    "--ncols-override" => ("NColsOverride", "NCOLS_OVERRIDE"),
                    _ => ("NRowsOverride", "NROWS_OVERRIDE"),
                };
                let _ = writeln!(r.out, "{label} {n}");
                r.env.set(var, n.to_string());
            }
            "--mosaic-fix-noascii" => r.env.set("FS_MOSAIC_FIX_NOASCII", "1"),
            "--nspmzeropad" | "--out_nspmzeropad" | "--in_nspmzeropad" => {
                let input = arg == "--in_nspmzeropad";
                if argc - 1 - i < 1 {
                    let name = if input {
                        "--in_nspmzeropad"
                    } else {
                        "--out_nspmzeropad"
                    };
                    return Err(fail_err(
                        r,
                        &format!("ERROR: option {name} requires one argument\n"),
                    ));
                }
                i += 1;
                if let Some(v) = sscanf_d(g, &a[i]) {
                    if input {
                        g.n_zero_pad_input = v;
                    } else {
                        g.n_zero_pad_output = v;
                    }
                }
            }
            "--no-strip-pound" => g.mriio_strip_pound = 0,
            "-zgez" | "--zero_ge_z_offset" => o.zero_ge_z_offset_flag = 1,
            "-nozgez" | "--no_zero_ge_z_offset" => o.zero_ge_z_offset_flag = 0,
            "--nskip" | "--ndrop" => {
                let n = args.int(g, r, &mut i)?;
                let name = &arg[2..];
                let _ = writeln!(r.out, "{name} = {n}");
                if n < 0 {
                    return Err(fail_out(
                        r,
                        &format!("ERROR: {name} cannot be negative\n"),
                    ));
                }
                if arg == "--nskip" {
                    o.nskip = n;
                } else {
                    o.ndrop = n;
                }
            }
            "--diag" => g.gdiag_no = args.int(g, r, &mut i)?,
            "--diag-debug" => g.gdiag |= DIAG_INFO,
            "--mra" => {
                o.dicom.slice_res_el_tag1 = 0x50;
                o.dicom.slice_res_el_tag2 = 0x88;
            }
            "--auto-slice-res" => o.dicom.auto_slice_res_el_tag = 1,
            _ => {
                if arg.starts_with('-') {
                    return Err(fail_usage(
                        g,
                        r,
                        &format!("\n{}: unknown flag \"{arg}\"\n", g.progname),
                    ));
                }
                if o.in_name.is_empty() {
                    fits(g, "the input volume's name", arg, STRLEN);
                    o.in_name = arg.to_owned();
                } else if o.out_name.is_empty() {
                    fits(g, "the output volume's name", arg, STRLEN);
                    o.out_name = arg.to_owned();
                } else if i + 1 == argc {
                    return Err(fail_usage(
                        g,
                        r,
                        &format!("\n{}: extra argument (\"{arg}\")\n", g.progname),
                    ));
                } else {
                    return Err(fail_usage(
                        g,
                        r,
                        &format!(
                            "\n{}: extra arguments (\"{arg}\" and following)\n",
                            g.progname
                        ),
                    ));
                }
            }
        }
        i += 1;
    }
    inconsistencies(r, &o)?;
    Ok(o)
}

/// The axis an option names by its `i`, `j` or `k` (`-iis`, `--in_j_size`, `-onk`, ...).
fn axis(arg: &str) -> usize {
    let b = arg.trim_start_matches('-').as_bytes();
    // the short forms put the axis second (`iis`, `oni`, `ijc`) or last (`onj`); the long ones
    // after the first underscore (`in_j_size`, `out_k_count`)
    let c = if let Some(n) = arg.find('_') {
        arg.as_bytes()[n + 1]
    } else if b.len() == 3 && (b[..2] == *b"on" || b[..2] == *b"in") {
        b[2]
    } else {
        b[1]
    };
    match c {
        b'i' => 0,
        b'j' => 1,
        _ => 2,
    }
}

/// `sqrt(log(256.0))`, which GCC folds at compile time (correctly rounded).
fn sqrt_log_256() -> f64 {
    larmorx_core::math::log(256.0).sqrt()
}

/// The option inconsistency checks after the loop (`mri_convert.cpp` 1625-1676).
fn inconsistencies(r: &mut Run, o: &Options) -> Result<(), Fatal> {
    let d = &o.dicom;
    if d.use_dcm2niix == 0
        && (d.dcm2niix_outdir.is_some()
            || d.dcm2niix_create_bids != 0
            || d.dcm2niix_no_force_stack_same_series != 0
            || d.dcm2niix_info_dump.is_some())
    {
        return Err(fail_err(
            r,
            "ERROR: option --dcm2niix-outdir <>, -dcm2niix-createBIDS, -dcm2niix-no-ForceStackSameSeries, --dcm2niix-info-dump <> are only valid with -dcm2niix\n",
        ));
    }
    if d.dcm2niix_create_bids != 0 && d.dcm2niix_outdir.is_none() {
        return Err(fail_err(
            r,
            "ERROR: use --dcm2niix-outdir <> to specify output directory for BIDS\n",
        ));
    }
    if o.antialias_flag != 0 && o.voxel_size_flag == 0 {
        return Err(fail_err(
            r,
            "ERROR: use --voxsize to specify voxel size for --antialias\n",
        ));
    }
    if o.antialias_flag != 0 && o.fwhm > 0.0 {
        return Err(fail_err(
            r,
            "ERROR: --antialias and --fwhm are mutually excluded!\n       use --antialias to automatically calculate FWHM, \n       or, --fwhm to specify desired FWHM\n",
        ));
    }
    if o.force_ras_good != 0 && o.in_direction_flag.iter().any(|&f| f != 0) {
        return Err(fail_err(
            r,
            "ERROR: cannot use --force_ras_good and --in_?_direction_flag\n",
        ));
    }
    if o.conform_flag == 0 && o.conform_min != 0 {
        return Err(fail_err(
            r,
            "In order to use -cm (--conform_min), you must set -c (--conform)  at the same time.\n",
        ));
    }
    if o.downsample_flag != 0 && o.downsampleold_flag != 0 {
        return Err(fail_err(
            r,
            "ERROR: --downsample and --downsampleold are mutually exclusive\n",
        ));
    }
    Ok(())
}

/// The checks before the volume is read (`mri_convert.cpp` 1678-1882): voxel sizes greater than
/// 0, an input and an output name, a template for `--out_stats_table`, the input's name without
/// `@` and `#` parts, three zeros of padding for `spm` input without `--in_nspmzeropad`, known
/// forced formats, the warnings of `--read_only`, the output's format (`mri_identify` of its name
/// unless forced; a COR output conforms), the parse-only report (status 0), a GDF output's image
/// stem.
pub fn check_options(g: &mut Globals, r: &mut Run, o: &mut Options) -> Result<(), Fatal> {
    let bad = |flag: i32, v: Option<f32>| flag != 0 && v.is_some_and(|v| v <= 0.0);
    let sizes = [
        ("in i", o.in_size_flag[0], o.in_size[0]),
        ("in j", o.in_size_flag[1], o.in_size[1]),
        ("in k", o.in_size_flag[2], o.in_size[2]),
        ("out i", o.out_size_flag[0], o.out_size[0]),
        ("out j", o.out_size_flag[1], o.out_size[1]),
        ("out k", o.out_size_flag[2], o.out_size[2]),
    ];
    let good = !sizes.iter().any(|&(_, f, v)| bad(f, v));
    if !good {
        let _ = write!(
            r.err,
            "\n{}: voxel sizes must be greater than zero\n",
            g.progname
        );
    }
    for &(name, f, v) in &sizes {
        if bad(f, v) {
            let _ = writeln!(r.err, "{name} size = {}", gf(v.unwrap_or(0.0)));
        }
    }
    if !good {
        usage_message(g, r.out);
        return Err(Fatal::Exit(1));
    }
    if o.in_name.is_empty() {
        return Err(fail_usage(
            g,
            r,
            &format!("\n{}: missing input volume name\n", g.progname),
        ));
    }
    if o.out_name.is_empty() && o.read_only_flag == 0 && o.no_write_flag == 0 {
        return Err(fail_usage(
            g,
            r,
            &format!("\n{}: missing output volume name\n", g.progname),
        ));
    }
    if o.out_stat_table_flag != 0 && o.out_like_flag == 0 {
        return Err(fail_usage(
            g,
            r,
            &format!(
                "\n{}: pass example (or empty) stats-table with --like to specify measure, column and row headers\n",
                g.progname
            ),
        ));
    }
    o.in_name_only = Some(mri_get_volume_name(g, &o.in_name));
    if o.in_type_string == "spm" && g.n_zero_pad_input < 0 {
        g.n_zero_pad_input = 3;
    }
    if o.force_in_type_flag != 0 && o.forced_in_type == MRI_VOLUME_TYPE_UNKNOWN {
        return Err(fail_usage(
            g,
            r,
            &format!(
                "\n{}: unknown input volume type {}\n",
                g.progname, o.in_type_string
            ),
        ));
    }
    if o.force_template_type_flag != 0 && o.forced_template_type == MRI_VOLUME_TYPE_UNKNOWN {
        let t = o.template_type_string.clone().unwrap_or_default();
        return Err(fail_usage(
            g,
            r,
            &format!("\n{}: unknown template volume type {t}\n", g.progname),
        ));
    }
    if o.force_out_type_flag != 0 && o.forced_out_type == MRI_VOLUME_TYPE_UNKNOWN && o.ascii_flag == 0
    {
        let t = o.out_type_string.clone().unwrap_or_default();
        return Err(fail_usage(
            g,
            r,
            &format!("\n{}: unknown output volume type {t}\n", g.progname),
        ));
    }
    if o.read_only_flag != 0 && !o.out_name.is_empty() {
        let _ = writeln!(
            r.err,
            "{}: warning: read only flag is set; nothing will be written to {}",
            g.progname, o.out_name
        );
    }
    if o.read_only_flag != 0 && (o.out_info_flag != 0 || o.out_matrix_flag != 0) {
        let _ = writeln!(
            r.err,
            "{}: warning: read only flag is set; no output information will be printed",
            g.progname
        );
    }
    if o.read_only_flag == 0 {
        if o.force_out_type_flag == 0 && o.out_stat_table_flag == 0 {
            o.out_volume_type = mri_identify(g, r.dir, r.env, r.out, r.err, &o.out_name)?;
            if o.out_volume_type == MRI_VOLUME_TYPE_UNKNOWN {
                return Err(fail_err(
                    r,
                    &format!("{}: can't determine type of output volume\n", g.progname),
                ));
            }
        } else {
            o.out_volume_type = o.forced_out_type;
        }
        if o.out_volume_type == MRI_CORONAL_SLICE_DIRECTORY {
            o.conform_flag = 1;
        }
    }
    if o.parse_only_flag != 0 {
        parse_only_report(r, o);
        return Err(Fatal::Exit(0));
    }
    Ok(())
}

/// The parse-only report (`-po`).
fn parse_only_report(r: &mut Run, o: &Options) {
    let w = &mut *r.out;
    let _ = writeln!(w, "input volume name: {}", o.in_name);
    let _ = writeln!(
        w,
        "input name only: {}",
        o.in_name_only.as_deref().unwrap_or("")
    );
    let _ = writeln!(w, "output volume name: {}", o.out_name);
    let _ = writeln!(w, "parse_only_flag = {}", o.parse_only_flag);
    let _ = writeln!(w, "conform_flag = {}", o.conform_flag);
    let _ = writeln!(
        w,
        "conform_size = {}",
        cfmt::f(f64::from(o.conform_size), 6)
    );
    let _ = writeln!(w, "in_info_flag = {}", o.in_info_flag);
    let _ = writeln!(w, "out_info_flag = {}", o.out_info_flag);
    let _ = writeln!(w, "in_matrix_flag = {}", o.in_matrix_flag);
    let _ = writeln!(w, "out_matrix_flag = {}", o.out_matrix_flag);
    if o.force_in_type_flag != 0 {
        let _ = writeln!(w, "input type is {}", o.forced_in_type);
    }
    if o.force_out_type_flag != 0 {
        let _ = writeln!(w, "output type is {}", o.forced_out_type);
    }
    if !o.subject_name.is_empty() {
        let _ = writeln!(w, "subject name is {}", o.subject_name);
    }
    if o.invert_val >= 0.0 {
        let _ = writeln!(w, "inversion, value is {}", gf(o.invert_val));
    }
    if o.reorder_flag != 0 {
        let v = o.reorder_vals.unwrap_or_default();
        let _ = writeln!(w, "reordering, values are {} {} {}", v[0], v[1], v[2]);
    }
    if o.reorder4_flag != 0 {
        let v = o.reorder4_vals.unwrap_or_default();
        let _ = writeln!(
            w,
            "reordering, values are {} {} {} {}",
            v[0], v[1], v[2], v[3]
        );
    }
    let _ = writeln!(
        w,
        "translation of otl labels is {}",
        if o.translate_labels_flag != 0 {
            "on"
        } else {
            "off"
        }
    );
}

/// The GDF check after the parse-only report: a GDF output needs `--gdf_image_stem`.
pub fn check_gdf_stem(g: &Globals, r: &mut Run, o: &Options) -> Result<(), Fatal> {
    if o.out_volume_type == GDF_FILE && o.gdf_image_stem.is_empty() {
        return Err(fail_err(
            r,
            &format!(
                "{}: GDF output type, but no GDF image file stem\n",
                g.progname
            ),
        ));
    }
    Ok(())
}

/// [`parse_options`] and [`check_options`]: what the command line sets, or the end of the program.
pub fn parse(g: &mut Globals, r: &mut Run, argv: &[String]) -> Result<Options, Fatal> {
    let mut o = parse_options(g, r, argv)?;
    check_options(g, r, &mut o)?;
    Ok(o)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strtol_as_glibc() {
        assert_eq!(strtol(b" 12x"), (12, 3));
        assert_eq!(strtol(b"-"), (0, 0));
        assert_eq!(strtol(b"+7"), (7, 2));
        assert_eq!(strtol(b"99999999999999999999"), (i64::MAX, 20));
        assert_eq!(strtol(b"-99999999999999999999").0, i64::MIN);
        assert_eq!(strtol(b""), (0, 0));
    }

    #[test]
    fn axes_of_the_options() {
        for (o, k) in [
            ("-iis", 0),
            ("--in_j_size", 1),
            ("-oks", 2),
            ("-oni", 0),
            ("-oic", 0),
            ("-onj", 1),
            ("-ojc", 1),
            ("--out_k_count", 2),
            ("-ini", 0),
            ("-ijc", 1),
            ("-ink", 2),
            ("-ikc", 2),
            ("-iid", 0),
            ("-ojd", 1),
            ("--in_k_direction", 2),
            ("--out_i_direction", 0),
        ] {
            assert_eq!(axis(o), k, "{o}");
        }
    }

    #[test]
    fn the_constant_of_fwhm() {
        assert_eq!(sqrt_log_256().to_bits(), 2.3548200450309493f64.to_bits());
    }
}
