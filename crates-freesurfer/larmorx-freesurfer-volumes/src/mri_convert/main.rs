// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_convert`'s `main` after its option loop (`mri_convert.cpp` 1876-3848, queue item 3.2-05's
//! second half) and `applyGaussianFilter` (3860): the program's entry point [`run`] (registered
//! as `mri_convert` in the crate's `PROGRAMS`) and [`main`], which the probe calls.
//!
//! The order of `main` after [`super::options::parse`]: a GDF output's stem checked; `--in_like`'s
//! header read; the input's format (forced, or `mri_identify` of the name without its `@` and `#`
//! parts; unknown: `ERROR: file <f> does not exist` or `ERROR: cannot determine file type for <f>
//! `); `--roi` needs GE input, `--zero_ge_z_offset` DICOM input; `reading from <f>...`; a morph
//! input (`.m3z`) read, halved with `--downsample2`, written and the end; an outline input
//! (`.otl`) read by `MRIreadOtl` (its slice count from `--in_like` or `--in_k_count`, its colour
//! table from `--color_file`; nearest sampling, no scaling); a GE ROI read; otherwise the header
//! (`--read_only` with `--in_info` or `--in_matrix` and no `--in_stats`), the volume (`MRIreadType`
//! when forced, `MRIread`, `MRIreadEx` with `--nth-frame`, or a stats table). Then, in order: the
//! embedded colour table kept aside, `--upper_thresh`, `--outside_val`, `--upsample`,
//! `--slice-crop`, `--left-right-reverse`, `--left-right-swap-label`, `--left-right-reverse-pix`,
//! `--left-right-mirror`/`--left-right-keep`, `--flip-cols`, `--slice-reverse`, `--slice-bias`,
//! `--reduce`, `--antialias` or `--fwhm`, `--rescale`, `--store_orig_ras2vox`, the command line
//! added, `--scale`, `--zero_ge_z_offset`, the pulse parameters printed; `--in_like`'s header
//! applied; the header overrides of the command line (sizes, directions, `--delete-cmds`,
//! `--new-transform-fname`, orientation, centre, subject, TR, TE, TI, flip angle) with the
//! starts, ends and field of view after new sizes; the input's axes printed (a warning when
//! they are not orthogonal); `--in_info`, `--in_matrix`, `--in_stats`; the transform read
//! (`LTAreadEx` for linear ones; FSL's needs `--like`; `--devolvexfm`; `--invert_transform`); the
//! template (`--reslice_like`, `--conform*`, `--voxsize`, `--downsample*`, otherwise the input's
//! header) and its command-line overrides; `--template_info`; `--read_only` ends here; the
//! transform applied (linear: `LTAtransformInterp`, into `--like`'s grid or with `--out_center`;
//! a morph: `GCAMmorphToAtlas`, or `GCAMmorphFromAtlas` inverted) or `--like` alone; the type
//! changed to the template's (zeros kept zero); resliced to the template when the geometry
//! differs by more than 1e-5; `--invert_contrast`, `--shift`, `--reorder`, `--reorder4`; the GDF
//! stem; `--out_info`, `--out_matrix`, `--out_stats`; `--erode-seg`, `--dil-seg`; `--frame` /
//! `--mid-frame`, `--ndrop`, `--nskip`, `--fsubsample`, `--cutends`, `--crop`, `--rescale-voxel`,
//! `--out_scale`, `--sphinx`, `--autoalign`; the colour table (`--ctab`, or the embedded one put
//! back); and the output: a stats table, ASCII text, or the volume (`MRIwriteType` when forced,
//! else `MRIwrite`; `--split` one file per frame), unless `--no_write`; `--first-dicom`'s file.
//!
//! Defects (`larmorx_core::defects`; the mechanisms in `docs/findings/freesurfer-convert.md`):
//! - **FS-1043** (crash): `--left-right-reverse` when no axis of the input points left or right.
//! - **FS-1044** (memory): `--new-transform-fname` of 1,024 bytes or more.
//! - **FS-1045** (message): the warning about axes that are not orthogonal never checks the second
//!   and third axes (by default it does).
//! - **FS-1046** (memory): `--invert_transform` of a transform without a source geometry, whose
//!   name has 505 bytes or more.
//! - **FS-1047** (crash): `--like` whose volume cannot be read, when the volume is resampled to it.
//! - **FS-1048** (a stray file): an inverted morph writes `t.mgz` into the working folder (by
//!   default it does not).
//! - **FS-1049** (crash): `--ascii` (or `--ascii+crsf`, `--ascii-fcol`) where the file cannot be
//!   created.
//! - **FS-1050** (crash): `--out_stats_table` when the table cannot be made from the volume.
//! - **FS-1051** (memory): `--split` with a frame's name of 1,024 bytes or more.
//! - **FS-1052** (crash): a step that gives no volume where `main` goes on with one.

use std::io::Write;

use larmorx_core::cfmt;
use larmorx_core::defects::{
    FS_1043, FS_1044, FS_1045, FS_1046, FS_1047, FS_1048, FS_1049, FS_1050, FS_1051, FS_1052,
};
use larmorx_core::math::Libm;
use larmorx_freesurfer_core::affine::{
    AffineMatrix, float_to_int, get_affine_matrix, set_affine_matrix,
};
use larmorx_freesurfer_core::cstd::note_glibc07;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_NOFILE, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{Matrix, matrix_multiply};
use larmorx_freesurfer_core::matrix_ops::{matrix_inverse, matrix_print};
use larmorx_freesurfer_core::pathsep;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::{Environment, Settings};
use larmorx_freesurfer_core::sse::{add, add64, div, div64, mul, mul64, sub, sub64};
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_filter::conv::{
    Smni, mri_gaussian_smooth, mri_gaussian_smooth_ni, mri_reduce, mri_reduce2d,
};
use larmorx_freesurfer_filter::fmri::{fmri_ndrop, fmri_nskip, fmri_sub_sample};
use larmorx_freesurfer_gca::gcam::{gcam_downsample2, gcam_read};
use larmorx_freesurfer_gca::gcam_write::gcam_write;
use larmorx_freesurfer_gca::morph::{gcam_morph_from_atlas, gcam_morph_to_atlas};
use larmorx_freesurfer_gca::transform::{GcaLtaIo, Xform, transform_free, transform_read};
use larmorx_freesurfer_io::colortab::ctab_read_ascii;
use larmorx_freesurfer_io::fio::fio_file_exists_readable;
use larmorx_freesurfer_io::identify::{
    DICOM_FILE, GENESIS_FILE, MGH_MORPH, MRI_VOLUME_TYPE_UNKNOWN, OTL_FILE, id_extension_from_name,
    id_stem_from_name, mri_identify,
};
use larmorx_freesurfer_io::mriio::{
    mri_add_command_line, mri_read_ex, mri_read_header, mri_read_info, mri_read_type,
    mri_read_volume, mri_write, mri_write_type,
};
use larmorx_freesurfer_io::mriio_ge::mri_read_ge_roi;
use larmorx_freesurfer_io::reorder::{mri_reorder, mri_reorder4};
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::ctab::ctab_deep_copy;
use larmorx_freesurfer_mri::info::mri_dump;
use larmorx_freesurfer_mri::lta::{
    FSLREG_TYPE, LINEAR_RAS_TO_RAS, LINEAR_VOX_TO_VOX, Lta, MNI_TRANSFORM_TYPE, MORPH_3D_TYPE,
    REGISTER_DAT, TRANSFORM_ARRAY_TYPE,
};
use larmorx_freesurfer_mri::mri::{
    MGZ_INTENT_UNKNOWN, Mri, SAMPLE_CUBIC, SAMPLE_NEAREST, SAMPLE_TRILINEAR, SAMPLE_VOTE,
    SAMPLE_WEIGHTED, mri_alloc, mri_alloc_header, mri_alloc_sequence, mri_copy_header,
    mri_copy_pulse_parameters, use_vol_geom_to_mri,
};
use larmorx_freesurfer_mri::tkreg::hfs2sphinx;
use larmorx_freesurfer_mri::vol_geom::{
    VolGeom, extract_i_to_r, extract_r_to_i, vgprint, xfm_crs2xyz,
};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::arith::{
    Pair, mri_copy_frame, mri_extract, mri_invert_contrast, mri_scalar_mul,
    mri_upper_threshold_all_frames,
};
use larmorx_freesurfer_volume::cma::{mri_lrswap_aseg, mri_lrswap_seg};
use larmorx_freesurfer_volume::conform::mri_conformed_template;
use larmorx_freesurfer_volume::conformed::{mri_find_min_size, mri_find_right_size};
use larmorx_freesurfer_volume::coords::dircos_to_orientation_string;
use larmorx_freesurfer_volume::labels::mri_smooth_parcellation;
use larmorx_freesurfer_volume::ltaapply::{
    devolve_xfm, lta_modify_src_dst_geom, lta_transform_interp,
};
use larmorx_freesurfer_volume::ltaops::lta_change_type;
use larmorx_freesurfer_volume::ltaread::{lta_read_ex, transform_file_name_type};
use larmorx_freesurfer_volume::mri2ops::mri_shift_dim;
use larmorx_freesurfer_volume::mriset::{mri_dilate_segmentation, mri_erode_segmentation};
use larmorx_freesurfer_volume::orient::orientation_string_to_dircos;
use larmorx_freesurfer_volume::resample::mri_resample;
use larmorx_freesurfer_volume::resize::{mri_downsample_n, mri_seq_change_type, mri_upsample_n};
use larmorx_freesurfer_volume::stats::{mri_print_stats, mri_type2str};
use larmorx_freesurfer_volume::vgeom::get_vol_geom;
use larmorx_freesurfer_volume::voxops::{mri_crop, mri_cut_end_slices, mri_reverse_slices};
use larmorx_freesurfer_volume::xform::{
    mri_get_resample_matrix, mri_linear_transform_interp, mri_ras_xform_to_voxel_xform,
};

use super::lta_dump::{lta_dump, lta_transform_type_name};
use super::options::{Options, Run, STRLEN, check_gdf_stem, parse};
use super::otl::{
    READ_OTL_FILL_FLAG, READ_OTL_READ_VOLUME_FLAG, READ_OTL_TRANSLATE_LABELS_FLAG,
    READ_OTL_ZERO_OUTLINES_FLAG, mri_read_otl,
};
use super::stat_table::{Io, init_stat_table_from_mri, load_stat_table, write_stat_table};

/// The program's name as `argv[0]` gives it (recon-all runs it by name).
pub const NAME: &str = "mri_convert";

/// `CLOSE_ENOUGH`: the largest dot product of two axes the program calls orthogonal.
const CLOSE_ENOUGH: f64 = 5e-3;

/// `mri_convert`'s entry point: its original arguments (without the program's name), the
/// settings, the two streams; the exit status. The notices of the run follow on standard error
/// (`larmorx: ...`).
pub fn run(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mut g = Globals::new(settings.mode);
    let mut env: Environment = settings.env.clone();
    let argv: Vec<String> = std::iter::once(NAME.to_owned())
        .chain(args.iter().cloned())
        .collect();
    let status = {
        let mut r = Run {
            out: &mut *out,
            err: &mut *err,
            env: &mut env,
            dir: settings.dir.as_deref(),
        };
        match main(&mut g, &mut r, &argv) {
            Ok(()) => 0,
            Err(f) => f.status(),
        }
    };
    let _ = out.flush();
    g.notices.print(err);
    status
}

/// `main(argc, argv)` (`argv[0]` the program's name): the options ([`parse`]), then the rest of
/// the program ([`convert`]). `Ok` is status 0.
pub fn main(g: &mut Globals, r: &mut Run, argv: &[String]) -> Result<(), Fatal> {
    let o = parse(g, r, argv)?;
    check_gdf_stem(g, r, &o)?;
    convert(g, r, o)
}

/// `%g` of a `float`.
fn gf(v: f32) -> String {
    cfmt::g(f64::from(v), 6)
}

/// `%f` of a `float` or `double`.
fn ff(v: f64) -> String {
    cfmt::f(v, 6)
}

/// `%<w>.<p>f`.
fn fwp(v: f64, w: usize, p: usize) -> String {
    cfmt::pad(&cfmt::f(v, p), w)
}

/// FS-1052: a volume `main` goes on with after a step that gave none.
fn need<T>(g: &mut Globals, err: &mut dyn Write, v: Option<T>, step: &str) -> Result<T, Fatal> {
    match v {
        Some(v) => Ok(v),
        None => {
            g.notices.add(
                &FS_1052,
                format!("mri_convert: {step} gave no volume, which FreeSurfer uses (a crash)"),
            );
            let _ = writeln!(
                err,
                "mri_convert: {step} gave no volume (FreeSurfer crashes here)"
            );
            Err(Fatal::Exit(1))
        }
    }
}

/// A function's volume: the new one, or the caller's when it filled it.
fn kept(r: Ret<Mri>, given: Mri) -> Option<Mri> {
    match r {
        Ret::New(m) => Some(m),
        Ret::Given => Some(given),
        Ret::Null => None,
    }
}

/// The three dot products of a geometry's axes (`float` arithmetic in the C expression's order).
fn dots(vg: &VolGeom) -> (f32, f32, f32) {
    let d = |a: [f32; 3], b: [f32; 3]| add(add(mul(a[0], b[0]), mul(a[1], b[1])), mul(a[2], b[2]));
    let x = [vg.x_r, vg.x_a, vg.x_s];
    let y = [vg.y_r, vg.y_a, vg.y_s];
    let z = [vg.z_r, vg.z_a, vg.z_s];
    (d(x, y), d(x, z), d(y, z))
}

/// Whether the warning about axes that are not orthogonal is printed: FreeSurfer tests the
/// first and second products and the second again (FS-1045: by default the third).
fn not_orthogonal(g: &mut Globals, ij: f32, ik: f32, jk: f32, which: &str) -> bool {
    let big = |v: f32| f64::from(v.abs()) > CLOSE_ENOUGH;
    let official = big(ij) || big(ik) || big(ik);
    let right = big(ij) || big(ik) || big(jk);
    if official != right {
        g.notices.add(
            &FS_1045,
            format!(
                "mri_convert: the {which} volume's second and third axes are not orthogonal (j_dot_k = {}), which FreeSurfer's warning never checks",
                cfmt::f(f64::from(jk), 6)
            ),
        );
        if !g.notices.reproduce() {
            return right;
        }
    }
    official
}

/// The `float` field of view after new voxel sizes or counts: the starts and ends at minus and
/// plus half of each axis's extent, the field of view the largest.
fn fix_fov(m: &mut Mri) {
    let fov_x = mul(m.xsize, m.width as f32);
    let fov_y = mul(m.ysize, m.height as f32);
    let fov_z = mul(m.zsize, m.depth as f32);
    m.xend = div64(f64::from(fov_x), 2.0) as f32;
    m.xstart = -m.xend;
    m.yend = div64(f64::from(fov_y), 2.0) as f32;
    m.ystart = -m.yend;
    m.zend = div64(f64::from(fov_z), 2.0) as f32;
    m.zstart = -m.zend;
    m.fov = if fov_x > fov_y {
        if fov_x > fov_z { fov_x } else { fov_z }
    } else if fov_y > fov_z {
        fov_y
    } else {
        fov_z
    };
}

/// `MatrixPrint(stdout, m)`.
fn mprint(g: &mut Globals, r: &mut Run, m: Option<&Matrix>) {
    let mut sink = std::io::sink();
    let out: &mut dyn Write = &mut *r.out;
    matrix_print(g, Some(out), &mut sink, r.err, m, 5);
}

/// The voxel `(c, r, s, f)` of `src` into `(c2, r2, s2, f)` of `dst`.
#[allow(clippy::too_many_arguments)]
fn copy_voxel(
    g: &mut Globals,
    err: &mut dyn Write,
    src: &Mri,
    dst: &mut Mri,
    from: [i32; 4],
    to: [i32; 3],
) -> Result<(), Fatal> {
    let v = mri_get_vox_val(g, err, src, from[0], from[1], from[2], from[3], 0)?;
    mri_set_vox_val(g, err, dst, to[0], to[1], to[2], from[3], v, 0)?;
    Ok(())
}

/// `MRIcopy(mri, NULL)`.
fn copy_of(g: &mut Globals, r: &mut Run, mri: &Mri) -> Result<Mri, Fatal> {
    let c = mri_copy(g, r.out, r.err, mri, None)?.into_new();
    need(g, r.err, c, "MRIcopy")
}

/// The rest of `main` after the option loop and its checks (`mri_convert.cpp` 1884-3848).
#[allow(clippy::cognitive_complexity)]
pub fn convert(g: &mut Globals, r: &mut Run, mut o: Options) -> Result<(), Fatal> {
    let in_name = o.in_name.clone();
    let in_name_only = o.in_name_only.clone().unwrap_or_default();
    let out_name = o.out_name.clone();
    let in_like_name = o.in_like_name.clone().unwrap_or_default();
    let out_like_name = o.out_like_name.clone().unwrap_or_default();
    let transform_fname = o.transform_fname.clone().unwrap_or_default();

    // ----- read the in_like volume -----
    let mut mri_in_like: Option<Mri> = None;
    if o.in_like_flag != 0 {
        let _ = writeln!(r.out, "reading info from {in_like_name}...");
        mri_in_like = mri_read_info(g, r.out, r.err, r.env, r.dir, &in_like_name)?;
        if mri_in_like.is_none() {
            return Err(Fatal::Exit(1));
        }
    }

    // ----- read the volume -----
    let mut in_volume_type = MRI_VOLUME_TYPE_UNKNOWN;
    if o.in_stat_table_flag == 0 {
        in_volume_type = if o.force_in_type_flag != 0 {
            o.forced_in_type
        } else {
            mri_identify(g, r.dir, r.env, r.out, r.err, &in_name_only)?
        };
        if in_volume_type == MRI_VOLUME_TYPE_UNKNOWN {
            g.errno = 0;
            if fio_file_exists_readable(g, r.dir, &in_name_only) == 0 {
                let _ = writeln!(r.out, "ERROR: file {in_name_only} does not exist");
            } else {
                let _ = writeln!(
                    r.out,
                    "ERROR: cannot determine file type for {in_name_only} "
                );
            }
            return Err(Fatal::Exit(1));
        }
    }

    if o.roi_flag != 0 && in_volume_type != GENESIS_FILE {
        g.errno = 0;
        log::error_printf(g, r.err, ERROR_BADPARM, "rois must be in GE format");
        return Err(Fatal::Exit(1));
    }

    if o.zero_ge_z_offset_flag != 0 && in_volume_type != DICOM_FILE {
        o.zero_ge_z_offset_flag = 0;
        let _ = writeln!(
            r.err,
            "Not a GE dicom volume: -zgez = --zero_ge_z_offset option ignored."
        );
    }

    let _ = writeln!(r.out, "reading from {in_name_only}...");

    if in_volume_type == MGH_MORPH {
        let gcam = gcam_read(g, r.out, r.err, r.env, r.dir, &in_name_only)?;
        let Some(mut gcam) = gcam else {
            let m = format!(
                "{}: could not read input morph from {in_name_only}",
                g.progname
            );
            log::error_exit(g, r.err, ERROR_NOFILE, &m)?;
            return Err(Fatal::Exit(1));
        };
        let mut gcam_out = if o.downsample2_flag != 0 {
            gcam_downsample2(g, r.err, &gcam)?
        } else {
            gcam
        };
        let datainterp = larmorx_freesurfer_io::warpfield::WARPFIELD_DTFMT_ABS_CRS;
        let _ = gcam_write(
            g,
            r.out,
            r.err,
            r.env,
            r.dir,
            &mut gcam_out,
            &out_name,
            datainterp,
        )?;
        gcam = gcam_out;
        drop(gcam);
        return Err(Fatal::Exit(0));
    }

    let mut resample_type_val = o.resample_type_val;
    let mut no_scale_flag = o.no_scale_flag;
    let reading_header_only = o.read_only_flag != 0
        && (o.in_info_flag != 0 || o.in_matrix_flag != 0)
        && o.in_stats_flag == 0;

    let mri: Option<Mri> = if in_volume_type == OTL_FILE {
        if o.in_like_flag == 0 && o.in_n_flag[2] == 0 {
            g.errno = 0;
            log::error_printf(
                g,
                r.err,
                ERROR_BADPARM,
                "parcellation read: must specifya volume depth with either in_like or in_k_count",
            );
            return Err(Fatal::Exit(1));
        }
        if o.color_file_flag == 0 {
            g.errno = 0;
            log::error_printf(
                g,
                r.err,
                ERROR_BADPARM,
                "parcellation read: must specify acolor file name",
            );
            return Err(Fatal::Exit(1));
        }
        let mut read_otl_flags = 0;
        if !reading_header_only {
            read_otl_flags |= READ_OTL_READ_VOLUME_FLAG;
        }
        if o.fill_parcellation_flag != 0 {
            read_otl_flags |= READ_OTL_FILL_FLAG;
        } else {
            let _ = writeln!(
                r.out,
                "notice: unfilled parcellations currently unimplemented"
            );
            let _ = writeln!(r.out, "notice: filling outlines");
            read_otl_flags |= READ_OTL_FILL_FLAG;
        }
        if o.translate_labels_flag != 0 {
            read_otl_flags |= READ_OTL_TRANSLATE_LABELS_FLAG;
        }
        if o.zero_outlines_flag != 0 {
            read_otl_flags |= READ_OTL_ZERO_OUTLINES_FLAG;
        }
        let color = o.color_file_name.clone().unwrap_or_default();
        let (w, h, d) = match &mri_in_like {
            Some(l) => (l.width, l.height, l.depth),
            None => (0, 0, o.in_n[2].unwrap_or(0)),
        };
        let Some(mut m) = mri_read_otl(g, r, &in_name, w, h, d, &color, read_otl_flags)? else {
            return Err(Fatal::Exit(1));
        };
        if o.smooth_parcellation_flag != 0 {
            let _ = writeln!(r.out, "smoothing parcellation...");
            let count = o.smooth_parcellation_count.unwrap_or(0);
            let Some(m2) = mri_smooth_parcellation(g, r.out, r.err, &m, count)? else {
                return Err(Fatal::Exit(1));
            };
            m = m2;
        }
        resample_type_val = SAMPLE_NEAREST;
        no_scale_flag = 1;
        Some(m)
    } else if o.roi_flag != 0 {
        if o.in_like_flag == 0 && o.in_n_flag[2] == 0 {
            g.errno = 0;
            log::error_printf(
                g,
                r.err,
                ERROR_BADPARM,
                "roi read: must specify a volumedepth with either in_like or in_k_count",
            );
            return Err(Fatal::Exit(1));
        }
        let n = match &mri_in_like {
            Some(l) => l.depth,
            None => o.in_n[2].unwrap_or(0),
        };
        let m = mri_read_ge_roi(g, r.out, r.err, r.dir, &in_name, n)?;
        if m.is_none() {
            return Err(Fatal::Exit(1));
        }
        resample_type_val = SAMPLE_NEAREST;
        no_scale_flag = 1;
        m
    } else if reading_header_only {
        if o.force_in_type_flag != 0 {
            mri_read_header(g, r.out, r.err, r.env, r.dir, &in_name, in_volume_type)?
        } else {
            mri_read_info(g, r.out, r.err, r.env, r.dir, &in_name)?
        }
    } else if o.force_in_type_flag != 0 {
        mri_read_type(g, r.out, r.err, r.env, r.dir, &in_name, in_volume_type)?
    } else if o.nthframe < 0 {
        if o.in_stat_table_flag == 0 {
            mri_read_volume(g, r.out, r.err, r.env, r.dir, &in_name)?
        } else {
            let _ = writeln!(r.out, "Loading in stat table {in_name}");
            let mut io = Io {
                out: &mut *r.out,
                err: &mut *r.err,
                env: &*r.env,
                dir: r.dir,
            };
            match load_stat_table(g, &mut io, &in_name)? {
                Some(st) => st.mri,
                None => {
                    let _ = writeln!(r.out, "ERROR: loading y {in_name} as a stat table");
                    return Err(Fatal::Exit(1));
                }
            }
        }
    } else {
        mri_read_ex(g, r.out, r.err, r.env, r.dir, &in_name, o.nthframe)?
    };
    let Some(mut mri) = mri else {
        return Err(Fatal::Exit(1));
    };

    // copy the embedded colour table: it gets lost
    let ctembedded = if mri.ct.is_some() {
        ctab_deep_copy(g, r.err, mri.ct.as_ref())
    } else {
        None
    };

    if o.upper_thresh_flag != 0 {
        let t = mri_upper_threshold_all_frames(g, r.out, r.err, Pair::New(&mri), o.upper_thresh)?
            .into_new();
        mri = need(g, r.err, t, "MRIupperthresholdAllFrames")?;
    }

    if o.outside_val > 0 {
        mri.outside_val = f64::from(o.outside_val);
    }
    if o.upsample_flag != 0 {
        let _ = writeln!(r.out, "UpsampleFactor = {}", o.upsample_factor);
        let t = mri_upsample_n(g, r.out, r.err, &mri, None, o.upsample_factor)?.into_new();
        mri = need(g, r.err, t, "MRIupsampleN")?;
        vgprint(r.out, &mri.vg, true);
    }

    if o.slice_crop_flag != 0 {
        let _ = writeln!(
            r.out,
            "Cropping slices from {} to {}",
            o.slice_crop_start, o.slice_crop_stop
        );
        let m2 = mri_crop(
            g,
            r.out,
            r.err,
            &mri,
            [0, 0, o.slice_crop_start],
            [mri.width - 1, mri.height - 1, o.slice_crop_stop],
        )?;
        let Some(m2) = m2 else {
            return Err(Fatal::Exit(1));
        };
        mri = m2;
    }

    if o.left_right_reverse != 0 {
        left_right_reverse(g, r, &mut mri)?;
    }

    if o.left_right_swap_label != 0 {
        let _ = writeln!(r.out, "Performing left-right swap of labels");
        let m2 = match (&o.lhswap, &o.rhswap) {
            (Some(lh), rh) => {
                let rh = rh.clone().unwrap_or_else(|| lh.clone());
                mri_lrswap_seg(g, r.out, r.err, &mri, lh, &rh)?
            }
            (None, _) => mri_lrswap_aseg(g, r.out, r.err, &mri)?,
        };
        mri = need(g, r.err, m2, "MRIlrswapAseg")?;
    }

    if o.left_right_reverse_pix != 0 {
        let _ = write!(
            r.out,
            "WARNING: applying left-right reversal to the input pixels\nwithout changing geometry. This will likely make \nthe volume geometry WRONG, so make sure you know what you  \nare doing.\n"
        );
        let (_, ostr) = dircos_to_orientation_string(&mri.vg);
        let mut mri2 = copy_of(g, r, &mri)?;
        let lr = |c: u8| c == b'L' || c == b'R';
        if lr(ostr[0]) {
            let _ = writeln!(r.out, "  Reversing pixels for the columns");
            for c in 0..mri.width {
                let c2 = mri.width - c - 1;
                for rr in 0..mri.height {
                    for s in 0..mri.depth {
                        for f in 0..mri.nframes {
                            copy_voxel(g, r.err, &mri, &mut mri2, [c, rr, s, f], [c2, rr, s])?;
                        }
                    }
                }
            }
        }
        if lr(ostr[1]) {
            let _ = writeln!(r.out, "  Reversing pixels for the rows");
            for c in 0..mri.width {
                for rr in 0..mri.height {
                    let r2 = mri.height - rr - 1;
                    for s in 0..mri.depth {
                        for f in 0..mri.nframes {
                            copy_voxel(g, r.err, &mri, &mut mri2, [c, rr, s, f], [c, r2, s])?;
                        }
                    }
                }
            }
        }
        if lr(ostr[2]) {
            let _ = writeln!(r.out, "  Reversing pixels for the slices");
            for c in 0..mri.width {
                for rr in 0..mri.height {
                    for s in 0..mri.depth {
                        let s2 = mri.depth - s - 1;
                        for f in 0..mri.nframes {
                            copy_voxel(g, r.err, &mri, &mut mri2, [c, rr, s, f], [c, rr, s2])?;
                        }
                    }
                }
            }
        }
        mri = mri2;
    }

    if o.left_right_mirror_flag != 0 || o.left_right_keep_flag != 0 {
        mri = left_right_mirror(g, r, &o, mri)?;
    }

    if o.flip_cols != 0 {
        let _ = writeln!(r.out, "WARNING: flipping cols without changing geometry");
        let mut mri2 = copy_of(g, r, &mri)?;
        let _ = writeln!(r.out, "type {} {}", mri.mtype, mri2.mtype);
        for c in 0..mri.width {
            let c2 = mri.width - c - 1;
            for rr in 0..mri.height {
                for s in 0..mri.depth {
                    for f in 0..mri.nframes {
                        copy_voxel(g, r.err, &mri, &mut mri2, [c, rr, s, f], [c2, rr, s])?;
                    }
                }
            }
        }
        let _ = writeln!(r.out, "type {} {}", mri.mtype, mri2.mtype);
        mri = mri2;
    }

    if o.slice_reverse != 0 {
        let _ = writeln!(r.out, "Reversing slices, updating vox2ras");
        let m2 = mri_reverse_slices(g, r.out, r.err, r.env, &mri, None)?.into_new();
        let Some(m2) = m2 else {
            return Err(Fatal::Exit(1));
        };
        mri = m2;
    }

    if o.slice_bias != 0 {
        let _ = writeln!(
            r.out,
            "Applying Half-Cosine Slice Bias, Alpha = {}",
            gf(o.slice_bias_alpha)
        );
        half_cos_bias_in_place(g, r, &mut mri, f64::from(o.slice_bias_alpha))?;
    }

    if o.reduce > 0 {
        for _ in 0..o.reduce {
            let t = if mri.depth == 1 {
                mri_reduce2d(g, r.out, r.err, &mut mri, None)?.into_new()
            } else {
                mri_reduce(g, r.out, r.err, r.env, &mri, None)?.into_new()
            };
            mri = need(g, r.err, t, "MRIreduce")?;
        }
    }

    if o.antialias_flag != 0 || o.fwhm > 0.0 {
        if o.antialias_flag != 0 {
            let vs = o.voxel_size.unwrap_or([0.0; 3]);
            apply_gaussian_filter(g, r, &mut mri, vs)?;
        } else if o.fwhm > 0.0 {
            let _ = writeln!(
                r.out,
                "Smoothing input at fwhm = {}, gstd = {}",
                gf(o.fwhm),
                gf(o.gstd)
            );
            let t =
                mri_gaussian_smooth(g, r.out, r.err, Pair::Same(&mut mri), f64::from(o.gstd), 1)?;
            drop(t);
        }
    }

    if !feq(o.rescale_factor, 1.0) {
        // the global mean of the input rescaled to rescale_factor
        let mut globalmean = 0.0f64;
        for c in 0..mri.width {
            for rr in 0..mri.height {
                for s in 0..mri.depth {
                    for f in 0..mri.nframes {
                        let v = mri_get_vox_val(g, r.err, &mri, c, rr, s, f, 0)?;
                        globalmean = add64(globalmean, f64::from(v));
                    }
                }
            }
        }
        let n = mri
            .width
            .wrapping_mul(mri.height)
            .wrapping_mul(mri.depth)
            .wrapping_mul(mri.nframes);
        globalmean = div64(globalmean, f64::from(n));
        let _ = writeln!(
            r.out,
            "Global rescaling input mean from {} to {}",
            cfmt::g(globalmean, 6),
            gf(o.rescale_factor)
        );
        let k = div64(f64::from(o.rescale_factor), globalmean) as f32;
        let t = mri_scalar_mul(g, r.out, r.err, Pair::Same(&mut mri), k)?;
        drop(t);
    }

    let mut m_orig_ras2vox: Option<Matrix> = None;
    if o.store_orig_ras2vox_flag != 0 {
        let _ = writeln!(r.out, "copying original vox2ras");
        m_orig_ras2vox = extract_r_to_i(g, r.out, r.err, &mri.vg)?;
    }

    mri_add_command_line(g, r.err, &mut mri, o.cmdline.as_bytes());
    if !feq(o.scale_factor, 1.0) {
        let _ = writeln!(
            r.out,
            "scaling input volume by {}",
            fwp(f64::from(o.scale_factor), 2, 3)
        );
        let t = mri_scalar_mul(g, r.out, r.err, Pair::Same(&mut mri), o.scale_factor)?;
        drop(t);
    }

    if o.zero_ge_z_offset_flag != 0 {
        mri.c_s = 0.0;
    }

    let degrees = div64(mul64(mri.flip_angle, 180.0), std::f64::consts::PI);
    let _ = writeln!(
        r.out,
        "TR={}, TE={}, TI={}, flip angle={}",
        fwp(f64::from(mri.tr), 2, 2),
        fwp(f64::from(mri.te), 2, 2),
        fwp(f64::from(mri.ti), 2, 2),
        fwp(degrees, 2, 2)
    );
    if in_volume_type != OTL_FILE {
        if o.fill_parcellation_flag != 0 {
            let _ = writeln!(
                r.out,
                "fill_parcellation flag ignored on a non-parcellation read"
            );
        }
        if o.smooth_parcellation_flag != 0 {
            let _ = writeln!(
                r.out,
                "smooth_parcellation flag ignored on a non-parcellation read"
            );
        }
    }

    // ----- apply the in_like volume if it's been read -----
    if let Some(like) = mri_in_like.take() {
        if mri.width != like.width || mri.height != like.height || mri.depth != like.depth {
            g.errno = 0;
            log::error_printf(g, r.err, ERROR_BADPARM, "volume sizes do not match\n");
            let m = format!(
                "{in_name}: (width, height, depth, frames) = ({}, {}, {}, {})\n",
                mri.width, mri.height, mri.depth, mri.nframes
            );
            log::error_printf(g, r.err, ERROR_BADPARM, &m);
            let m = format!(
                "{in_like_name}: (width, height, depth, frames) = ({}, {}, {}, {})\n",
                like.width, like.height, like.depth, like.nframes
            );
            log::error_printf(g, r.err, ERROR_BADPARM, &m);
            return Err(Fatal::Exit(1));
        }
        if mri.nframes != like.nframes {
            let _ = writeln!(r.out, "INFO: frames are not the same");
        }
        let temp_type = mri.mtype;
        let c = mri_copy_header(g, r.out, r.err, &like, Some(&mut mri))?;
        if c.is_null() {
            g.errno = 0;
            let m = format!(
                "error copying information from {in_like_name} structure to {in_name} structure\n"
            );
            log::error_printf(g, r.err, ERROR_BADPARM, &m);
            return Err(Fatal::Exit(1));
        }
        mri.mtype = temp_type;
    }

    if mri.ras_good_flag == 0 {
        let _ = write!(
            r.out,
            "WARNING: it does not appear that there was sufficient information\nin the input to assign orientation to the volume... \n"
        );
        if o.force_ras_good != 0 {
            let _ = write!(
                r.out,
                "However, you have specified that the default orientation should\nbe used with by adding --force_ras_good on the command-line.\n"
            );
            mri.ras_good_flag = 1;
        }
        if o.in_direction_flag.iter().any(|&f| f != 0) {
            let _ = write!(
                r.out,
                "However, you have specified one or more orientations on the \ncommand-line using -i?d or --in-?-direction (?=i,j,k).\n"
            );
            mri.ras_good_flag = 1;
        }
    }

    // ----- apply command-line parameters -----
    if o.in_size_flag[0] != 0 {
        mri.xsize = o.in_size[0].unwrap_or(0.0);
    }
    if o.in_size_flag[1] != 0 {
        mri.ysize = o.in_size[1].unwrap_or(0.0);
    }
    if o.in_size_flag[2] != 0 {
        mri.zsize = o.in_size[2].unwrap_or(0.0);
    }
    if o.in_direction_flag[0] != 0 {
        let d = o.in_directions[0].unwrap_or([0.0; 3]);
        (mri.x_r, mri.x_a, mri.x_s) = (d[0], d[1], d[2]);
        mri.ras_good_flag = 1;
    }
    if o.in_direction_flag[1] != 0 {
        let d = o.in_directions[1].unwrap_or([0.0; 3]);
        (mri.y_r, mri.y_a, mri.y_s) = (d[0], d[1], d[2]);
        mri.ras_good_flag = 1;
    }
    if o.in_direction_flag[2] != 0 {
        let d = o.in_directions[2].unwrap_or([0.0; 3]);
        (mri.z_r, mri.z_a, mri.z_s) = (d[0], d[1], d[2]);
        mri.ras_good_flag = 1;
    }
    if o.delete_cmds != 0 {
        mri.cmdlines.clear();
    }
    if o.do_new_transform_fname != 0 {
        let _ = writeln!(r.out, "Changing xform name to {}", o.new_transform_fname);
        let name = o.new_transform_fname.as_bytes();
        if name.len() >= STRLEN {
            g.notices.add(
                &FS_1044,
                format!(
                    "mri_convert --new-transform-fname: a name of {} bytes copied into the volume's 1,024-byte field (FreeSurfer writes past it)",
                    name.len()
                ),
            );
        }
        mri.transform_fname = name.to_vec();
    }
    if o.in_orientation_flag != 0 {
        let s = o.in_orientation_string.clone().unwrap_or_default();
        let _ = writeln!(r.out, "Setting input orientation to {s}");
        orientation_string_to_dircos(g, r.out, &mut mri, s.as_bytes());
        mri.ras_good_flag = 1;
    }
    if o.in_center_flag != 0 {
        let c = o.in_center.unwrap_or([0.0; 3]);
        (mri.c_r, mri.c_a, mri.c_s) = (c[0], c[1], c[2]);
    }
    if o.delta_in_center_flag != 0 {
        let c = o.delta_in_center.unwrap_or([0.0; 3]);
        mri.c_r = add(mri.c_r, c[0]);
        mri.c_a = add(mri.c_a, c[1]);
        mri.c_s = add(mri.c_s, c[2]);
    }
    if !o.subject_name.is_empty() {
        mri.subject_name = o.subject_name.clone();
    }
    if o.in_tr_flag != 0 {
        mri.tr = o.in_tr;
    }
    if o.in_ti_flag != 0 {
        mri.ti = o.in_ti;
    }
    if o.in_te_flag != 0 {
        mri.te = o.in_te;
    }
    if o.in_flip_angle_flag != 0 {
        mri.flip_angle = f64::from(o.in_flip_angle);
    }

    // ----- correct starts, ends, and fov if necessary -----
    if o.in_size_flag.iter().any(|&f| f != 0) {
        fix_fov(&mut mri);
    }

    // ----- give a warning for non-orthogonal directions -----
    let (ij, ik, jk) = dots(&mri.vg);
    if not_orthogonal(g, ij, ik, jk, "input") {
        let _ = writeln!(
            r.out,
            "warning: input volume axes are not orthogonal:i_dot_j = {}, i_dot_k = {}, j_dot_k = {}",
            ff(f64::from(ij)),
            ff(f64::from(ik)),
            ff(f64::from(jk))
        );
    }
    let _ = writeln!(
        r.out,
        "i_ras = ({}, {}, {})",
        gf(mri.x_r),
        gf(mri.x_a),
        gf(mri.x_s)
    );
    let _ = writeln!(
        r.out,
        "j_ras = ({}, {}, {})",
        gf(mri.y_r),
        gf(mri.y_a),
        gf(mri.y_s)
    );
    let _ = writeln!(
        r.out,
        "k_ras = ({}, {}, {})",
        gf(mri.z_r),
        gf(mri.z_a),
        gf(mri.z_s)
    );

    // ----- catch the in info flag -----
    if o.in_info_flag != 0 {
        let _ = writeln!(r.out, "input structure:");
        dump(g, r, &mri)?;
    }
    if o.in_matrix_flag != 0 {
        let i_to_r = extract_i_to_r(g, r.out, r.err, &mri.vg)?;
        let _ = writeln!(r.out, "input ijk -> ras:");
        mprint(g, r, Some(&i_to_r));
    }
    if o.in_stats_flag != 0 {
        stats(g, r, &mri)?;
    }

    // ----- load the transform -----
    let mut transform_type = -1;
    let mut lta_transform: Option<Lta> = None;
    if o.transform_flag != 0 {
        let _ = writeln!(
            r.out,
            "INFO: Reading transformation from file {transform_fname}..."
        );
        transform_type = transform_file_name_type(g, transform_fname.as_bytes());
        if is_linear(transform_type) {
            let _ = writeln!(r.out, "Reading transform with LTAreadEx()");
            let mut io = GcaLtaIo { env: &*r.env };
            let lta = lta_read_ex(g, r.out, r.err, r.dir, &transform_fname, &mut io)?;
            let Some(mut lta) = lta else {
                let _ = writeln!(
                    r.err,
                    "ERROR: Reading transform from file {transform_fname}"
                );
                return Err(Fatal::Exit(1));
            };
            if transform_type == FSLREG_TYPE {
                if o.out_like_flag == 0 {
                    let _ = writeln!(
                        r.out,
                        "ERROR: fslmat does not have the information on the dst volume"
                    );
                    let _ = writeln!(
                        r.out,
                        "ERROR: you must give option '--like volume' to specify the dst volume info"
                    );
                    return Err(Fatal::Exit(1));
                }
                let tmp = mri_read_header(
                    g,
                    r.out,
                    r.err,
                    r.env,
                    r.dir,
                    &out_like_name,
                    MRI_VOLUME_TYPE_UNKNOWN,
                )?;
                lta_modify_src_dst_geom(g, r.out, r.err, &mut lta, Some(&mri), tmp.as_ref());
                lta_change_type(g, r.out, r.err, r.env, &mut lta, LINEAR_VOX_TO_VOX)?;
            }
            if o.dev_xfm != 0 {
                let subject = o.devxfm_subject.clone().unwrap_or_default();
                let _ = writeln!(r.out, "INFO: devolving XFM ({subject})");
                let _ = writeln!(r.out, "-------- before ---------");
                let m = lta.xforms[0].m_l.clone();
                mprint(g, r, m.as_ref());
                let mut io = GcaLtaIo { env: &*r.env };
                let t = devolve_xfm(
                    g,
                    r.out,
                    r.err,
                    r.dir,
                    &subject,
                    lta.xforms[0].m_l.as_mut(),
                    None,
                    &mut io,
                )?;
                if t.is_null() {
                    return Err(Fatal::Exit(1));
                }
                let _ = writeln!(r.out, "-------- after ---------");
                let m = lta.xforms[0].m_l.clone();
                mprint(g, r, m.as_ref());
                let _ = writeln!(r.out, "-----------------");
            }
            if o.invert_transform_flag != 0 {
                invert_lta(g, r, &mut lta, &transform_fname)?;
            }
            lta_transform = Some(lta);
        }
        // a non-linear transform is read when it is applied
    }

    // ----- the template -----
    let mut mri_template: Mri;
    if o.reslice_like_flag != 0 {
        let like = o.reslice_like_name.clone().unwrap_or_default();
        if o.force_template_type_flag != 0 {
            let _ = writeln!(
                r.out,
                "reading template info from (type {}) volume {like}...",
                o.template_type_string.clone().unwrap_or_default()
            );
            let t = mri_read_header(g, r.out, r.err, r.env, r.dir, &like, o.forced_template_type)?;
            let Some(t) = t else {
                let _ = writeln!(r.err, "error reading from volume {like}");
                return Err(Fatal::Exit(1));
            };
            mri_template = t;
        } else {
            let _ = writeln!(r.out, "reading template info from volume {like}...");
            let t = mri_read_info(g, r.out, r.err, r.env, r.dir, &like)?;
            let Some(t) = t else {
                let _ = writeln!(r.err, "error reading from volume {like}");
                return Err(Fatal::Exit(1));
            };
            mri_template = t;
            if let Some(lta) = lta_transform.as_mut() {
                lta_change_type(g, r.out, r.err, r.env, lta, LINEAR_RAS_TO_RAS)?;
                lta_modify_src_dst_geom(g, r.out, r.err, lta, None, Some(&mri_template));
            }
        }
    } else {
        mri_template = header_like(g, r, &mri)?;
        if let Some(lta) = lta_transform
            .as_ref()
            .filter(|l| l.xforms[0].dst.valid == 1)
        {
            let dst = lta.xforms[0].dst.clone();
            use_vol_geom_to_mri(g, r.out, r.err, &dst, &mut mri_template.vg)?;
        }
        if o.conform_flag != 0 {
            let (conform_size, conform_width) = if o.conform_min != 0 {
                mri_find_min_size(g, &mri.vg)
            } else if o.conform_width_256_flag != 0 {
                (o.conform_size, 256)
            } else {
                (
                    o.conform_size,
                    mri_find_right_size(g, r.err, &mri.vg, o.conform_size),
                )
            };
            mri_template = mri_conformed_template(
                g,
                r.out,
                r.err,
                &mri,
                conform_width,
                f64::from(conform_size),
                o.conf_keep_dc != 0,
            )?;
            vgprint(r.out, &mri_template.vg, true);
        } else if o.voxel_size_flag != 0 {
            let vs = o.voxel_size.unwrap_or([0.0; 3]);
            mri_template = header_like(g, r, &mri)?;
            mri_template.nframes = mri.nframes;
            let n = |g: &mut Globals, size: f32, n: i32, v: f32| {
                float_to_int(
                    g,
                    f64::from(div(mul(size, n as f32), v).ceil()),
                    "(int)ceil",
                )
            };
            mri_template.vg.width = n(g, mri.xsize, mri.width, vs[0]);
            mri_template.vg.height = n(g, mri.ysize, mri.height, vs[1]);
            mri_template.vg.depth = n(g, mri.zsize, mri.depth, vs[2]);
            mri_template.xsize = vs[0];
            mri_template.ysize = vs[1];
            mri_template.zsize = vs[2];
            mri_template.xstart = (-mri_template.width / 2) as f32;
            mri_template.xend = (mri_template.width / 2) as f32;
            mri_template.ystart = (-mri_template.height / 2) as f32;
            mri_template.yend = (mri_template.height / 2) as f32;
            mri_template.zstart = (-mri_template.depth / 2) as f32;
            mri_template.zend = (mri_template.depth / 2) as f32;
        } else if o.downsample_flag != 0 || o.downsampleold_flag != 0 {
            let df = o.downsample_factor.unwrap_or([0.0; 3]);
            if o.downsampleold_flag == 0 {
                let _ = writeln!(
                    r.out,
                    "DownsampleFactor = {}, {}, {}",
                    ff(f64::from(df[0])),
                    ff(f64::from(df[1])),
                    ff(f64::from(df[2]))
                );
                let t = mri_downsample_n(g, r.out, r.err, &mri, None, df[0], df[1], df[2], 1)?
                    .into_new();
                mri_template = need(g, r.err, t, "MRIdownsampleN")?;
            } else {
                // the old --downsample
                mri_template = header_like(g, r, &mri)?;
                mri_template.nframes = mri.nframes;
                let n = |g: &mut Globals, n: i32, f: f32| {
                    float_to_int(g, f64::from(div(n as f32, f).ceil()), "(int)ceil")
                };
                mri_template.vg.width = n(g, mri.width, df[0]);
                mri_template.vg.height = n(g, mri.height, df[1]);
                mri_template.vg.depth = n(g, mri.depth, df[2]);
                mri_template.xsize = mul(mri_template.xsize, df[0]);
                mri_template.ysize = mul(mri_template.ysize, df[1]);
                mri_template.zsize = mul(mri_template.zsize, df[2]);
                // the starts and ends do not change
                let half = |size: f32, n: i32| div(mul(-size, n as f32), 2.0);
                let halfp = |size: f32, n: i32| div(mul(size, n as f32), 2.0);
                let t = &mut mri_template;
                t.xstart = half(t.vg.xsize, t.vg.width);
                t.xend = halfp(t.vg.xsize, t.vg.width);
                t.ystart = half(t.vg.ysize, t.vg.height);
                t.yend = halfp(t.vg.ysize, t.vg.height);
                t.zstart = half(t.vg.zsize, t.vg.depth);
                t.zend = halfp(t.vg.zsize, t.vg.depth);
            }
            vgprint(r.out, &mri_template.vg, true);
        }
    }

    // ----- apply command-line parameters -----
    for k in 0..3 {
        if o.out_size_flag[k] != 0 {
            let size = o.out_size[k].unwrap_or(0.0);
            let t = &mut mri_template;
            let (cur, n) = match k {
                0 => (t.vg.xsize, t.vg.width),
                1 => (t.vg.ysize, t.vg.height),
                _ => (t.vg.zsize, t.vg.depth),
            };
            let scale = div(cur, size);
            let newn = nint(g, f64::from(mul(n as f32, scale)));
            match k {
                0 => (t.vg.xsize, t.vg.width) = (size, newn),
                1 => (t.vg.ysize, t.vg.height) = (size, newn),
                _ => (t.vg.zsize, t.vg.depth) = (size, newn),
            }
        }
    }
    if o.out_n_flag[0] != 0 {
        mri_template.vg.width = o.out_n[0].unwrap_or(0);
    }
    if o.out_n_flag[1] != 0 {
        mri_template.vg.height = o.out_n[1].unwrap_or(0);
    }
    if o.out_n_flag[2] != 0 {
        let n = o.out_n[2].unwrap_or(0);
        mri_template.vg.depth = n;
        mri_template.imnr1 = mri_template.imnr0.wrapping_add(n).wrapping_sub(1);
    }
    if o.out_direction_flag[0] != 0 {
        let d = o.out_directions[0].unwrap_or([0.0; 3]);
        (mri_template.x_r, mri_template.x_a, mri_template.x_s) = (d[0], d[1], d[2]);
    }
    if o.out_direction_flag[1] != 0 {
        let d = o.out_directions[1].unwrap_or([0.0; 3]);
        (mri_template.y_r, mri_template.y_a, mri_template.y_s) = (d[0], d[1], d[2]);
    }
    if o.out_direction_flag[2] != 0 {
        let d = o.out_directions[2].unwrap_or([0.0; 3]);
        (mri_template.z_r, mri_template.z_a, mri_template.z_s) = (d[0], d[1], d[2]);
    }
    if o.out_orientation_flag != 0 {
        let s = o.out_orientation_string.clone().unwrap_or_default();
        let _ = writeln!(r.out, "Setting output orientation to {s}");
        orientation_string_to_dircos(g, r.out, &mut mri_template, s.as_bytes());
    }
    if o.out_center_flag != 0 {
        let c = o.out_center.unwrap_or([0.0; 3]);
        (mri_template.c_r, mri_template.c_a, mri_template.c_s) = (c[0], c[1], c[2]);
    }

    // ----- correct starts, ends, and fov if necessary -----
    if o.out_size_flag.iter().any(|&f| f != 0) || o.out_n_flag.iter().any(|&f| f != 0) {
        fix_fov(&mut mri_template);
    }

    // ----- give a warning for non-orthogonal directions -----
    let (ij, ik, jk) = dots(&mri_template.vg);
    if not_orthogonal(g, ij, ik, jk, "output") {
        let t = &mri_template;
        let _ = writeln!(
            r.out,
            "warning: output volume axes are not orthogonal:i_dot_j = {}, i_dot_k = {}, j_dot_k = {}",
            ff(f64::from(ij)),
            ff(f64::from(ik)),
            ff(f64::from(jk))
        );
        let _ = writeln!(
            r.out,
            "i_ras = ({}, {}, {})",
            gf(t.x_r),
            gf(t.x_a),
            gf(t.x_s)
        );
        let _ = writeln!(
            r.out,
            "j_ras = ({}, {}, {})",
            gf(t.y_r),
            gf(t.y_a),
            gf(t.y_s)
        );
        let _ = writeln!(
            r.out,
            "k_ras = ({}, {}, {})",
            gf(t.z_r),
            gf(t.z_a),
            gf(t.z_s)
        );
    }
    if o.out_data_type >= 0 {
        mri_template.mtype = o.out_data_type;
    }

    // ----- catch the mri_template info flag -----
    if o.template_info_flag != 0 {
        let _ = writeln!(r.out, "template structure:");
        dump(g, r, &mri_template)?;
    }

    // ----- exit here if read only is desired -----
    if o.read_only_flag != 0 {
        return Err(Fatal::Exit(0));
    }

    // ----- apply a transformation if requested -----
    if o.transform_flag != 0 {
        let _ = writeln!(
            r.out,
            "INFO: Applying transformation from file {transform_fname}..."
        );
        transform_type = transform_file_name_type(g, transform_fname.as_bytes());
        if is_linear(transform_type) {
            // the transform was read above
            let Some(mut lta) = lta_transform.take() else {
                let _ = writeln!(r.err, "ERROR: lta should have been loaded ");
                return Err(Fatal::Exit(1));
            };
            let _ = writeln!(r.out, "---------------------------------");
            let _ = writeln!(
                r.out,
                "INFO: Transform Matrix ({})",
                lta_transform_type_name(lta.ltype)
            );
            let m = lta.xforms[0].m_l.clone();
            mprint(g, r, m.as_ref());
            let _ = writeln!(r.out, "---------------------------------");
            let transformed = if o.out_like_flag == 1 {
                let _ = writeln!(
                    r.out,
                    "INFO: transform dst into the like-volume (resample_type {resample_type_val})"
                );
                let tmp = read_like(g, r, &out_like_name)?;
                let mut dst =
                    mri_alloc(g, r.out, r.err, tmp.width, tmp.height, tmp.depth, mri.mtype)?;
                let _ = mri_copy_header(g, r.out, r.err, &tmp, Some(&mut dst))?;
                drop(tmp);
                let t = lta_transform_interp(
                    g,
                    r.out,
                    r.err,
                    r.env,
                    &mri,
                    Some(&mut dst),
                    &mut lta,
                    resample_type_val,
                )?;
                kept(t, dst)
            } else {
                if o.out_center_flag != 0 {
                    let m = mri_get_resample_matrix(g, r.out, r.err, &mri_template, &mri)?;
                    let mtmp = mri_ras_xform_to_voxel_xform(
                        g,
                        r.out,
                        r.err,
                        &mri,
                        Some(&mri),
                        lta.xforms[0].m_l.as_ref(),
                    )?;
                    let p = matrix_multiply(g, r.out, r.err, m.as_ref(), mtmp.as_ref(), None)?;
                    lta.xforms[0].m_l = p.into_new();
                    lta.ltype = LINEAR_VOX_TO_VOX;
                }
                let _ = writeln!(
                    r.out,
                    "Applying LTAtransformInterp (resample_type {resample_type_val})"
                );
                if g.gdiag_no > 0 {
                    let _ = writeln!(
                        r.out,
                        "Dumping LTA ---------++++++++++++-----------------------"
                    );
                    lta_dump(g, r, &lta);
                    let _ = writeln!(
                        r.out,
                        "========-------------++++++++++++-------------------====="
                    );
                }
                let t = lta_transform_interp(
                    g,
                    r.out,
                    r.err,
                    r.env,
                    &mri,
                    None,
                    &mut lta,
                    resample_type_val,
                )?;
                t.into_new()
            };
            let Some(mut transformed) = transformed else {
                let _ = writeln!(r.err, "ERROR: applying transform to volume");
                return Err(Fatal::Exit(1));
            };
            if o.out_center_flag != 0 {
                transformed.c_r = mri_template.c_r;
                transformed.c_a = mri_template.c_a;
                transformed.c_s = mri_template.c_s;
            }
            drop(lta);
            mri = transformed;
        } else if transform_type == MORPH_3D_TYPE {
            let _ = writeln!(r.out, "Applying morph_3d ...");
            // a non-linear voxel-to-voxel transform
            let tran = transform_read(g, r.out, r.err, r.env, r.dir, &transform_fname)?;
            let Some(mut tran) = tran else {
                let m = format!(
                    "{}: could not read xform from {transform_fname}\n",
                    g.progname
                );
                log::error_exit(g, r.err, ERROR_NOFILE, &m)?;
                return Err(Fatal::Exit(1));
            };
            let transformed = match &mut tran.xform {
                Xform::Morph(gcam) => {
                    if o.invert_transform_flag == 0 {
                        let _ = writeln!(
                            r.out,
                            "morphing to atlas with resample type {resample_type_val}"
                        );
                        let t = gcam_morph_to_atlas(
                            g,
                            r.out,
                            r.err,
                            r.env,
                            &mri,
                            gcam,
                            None,
                            -1,
                            resample_type_val,
                        )?
                        .into_new();
                        let atlas = gcam.atlas.clone();
                        use_vol_geom_to_mri(g, r.out, r.err, &atlas, &mut mri_template.vg)?;
                        need(g, r.err, t, "GCAMmorphToAtlas")?
                    } else {
                        let _ = writeln!(
                            r.out,
                            "morphing from atlas with resample type {resample_type_val}"
                        );
                        let t = gcam_morph_from_atlas(
                            g,
                            r.out,
                            r.err,
                            r.env,
                            r.dir,
                            &mri,
                            gcam,
                            None,
                            resample_type_val,
                        )?
                        .into_new();
                        let mut t = need(g, r.err, t, "GCAMmorphFromAtlas")?;
                        write_t_mgz(g, r, &mut t)?;
                        t
                    }
                }
                Xform::Lta(_) => {
                    // TransformRead gives a morph for every name TransformFileNameType calls one
                    let m = format!(
                        "mri_convert: {transform_fname} was read as a linear transform where a morph was expected"
                    );
                    need::<Mri>(g, r.err, None, &m)?
                }
            };
            let mut tran = Some(tran);
            transform_free(g, &mut tran);
            mri = transformed;
        } else {
            let _ = writeln!(r.err, "unknown transform type in file {transform_fname}");
            return Err(Fatal::Exit(1));
        }
    } else if o.out_like_flag != 0 && o.out_stat_table_flag == 0 {
        // no transform: the input resampled into the --like volume's voxels
        let _ = writeln!(
            r.out,
            "INFO: transform src into the like-volume: {out_like_name}"
        );
        let tmp = read_like(g, r, &out_like_name)?;
        let mut dst = mri_alloc(g, r.out, r.err, tmp.width, tmp.height, tmp.depth, mri.mtype)?;
        let _ = mri_copy_header(g, r.out, r.err, &tmp, Some(&mut dst))?;
        drop(tmp);
        let tmp2 = match mri.vg.i_to_r {
            None => {
                let m = extract_i_to_r(g, r.out, r.err, &mri.vg)?;
                let mut a = AffineMatrix::default();
                set_affine_matrix(r.err, &mut a, &m)?;
                mri.vg.i_to_r = Some(a);
                m
            }
            Some(a) => {
                let mut m = Matrix::real(4, 4);
                get_affine_matrix(r.err, &mut m, &a)?;
                m
            }
        };
        if dst.vg.r_to_i.is_none() {
            dst.vg.r_to_i = extract_r_to_i(g, r.out, r.err, &dst.vg)?;
        }
        let src2dst =
            matrix_multiply(g, r.out, r.err, dst.vg.r_to_i.as_ref(), Some(&tmp2), None)?.into_new();
        let t = mri_linear_transform_interp(
            g,
            r.out,
            r.err,
            &mri,
            Some(&mut dst),
            src2dst.as_ref(),
            resample_type_val,
        )?;
        drop(t);
        mri = dst;
    }

    // ----- change type if necessary -----
    if mri.mtype != mri_template.mtype && o.nochange_flag == 0 {
        let _ = writeln!(
            r.out,
            "changing data type from {} to {} (noscale = {no_scale_flag})...",
            mri_type2str(mri.mtype),
            mri_type2str(mri_template.mtype)
        );
        let m2 = mri_seq_change_type(
            g,
            r.out,
            r.err,
            r.env,
            &mut mri,
            mri_template.mtype,
            0.0,
            0.999,
            no_scale_flag,
        )?;
        let Some(mut m2) = m2 else {
            let _ = writeln!(r.out, "ERROR: MRISeqchangeType");
            return Err(Fatal::Exit(1));
        };
        // 0 always maps to 0
        for c in 0..mri.width {
            for rr in 0..mri.height {
                for s in 0..mri.depth {
                    for f in 0..mri.nframes {
                        let v = mri_get_vox_val(g, r.err, &mri, c, rr, s, f, 0)?;
                        if v == 0.0 {
                            mri_set_vox_val(g, r.err, &mut m2, c, rr, s, f, 0.0, 0)?;
                        }
                    }
                }
            }
        }
        mri = m2;
    }

    // ----- reslice if necessary and not performed during transform -----
    let eps = 1e-05f32;
    let t = &mri_template;
    let far = |a: f32, b: f32| f64::from(sub(a, b).abs()) > f64::from(eps);
    if o.out_like_flag == 0
        && transform_type != MORPH_3D_TYPE
        && (far(mri.xsize, t.xsize)
            || far(mri.ysize, t.ysize)
            || far(mri.zsize, t.zsize)
            || mri.width != t.width
            || mri.height != t.height
            || mri.depth != t.depth
            || far(mri.x_r, t.x_r)
            || far(mri.x_a, t.x_a)
            || far(mri.x_s, t.x_s)
            || far(mri.y_r, t.y_r)
            || far(mri.y_a, t.y_a)
            || far(mri.y_s, t.y_s)
            || far(mri.z_r, t.z_r)
            || far(mri.z_a, t.z_a)
            || far(mri.z_s, t.z_s)
            || far(mri.c_r, t.c_r)
            || far(mri.c_a, t.c_a)
            || far(mri.c_s, t.c_s))
    {
        let _ = write!(r.out, "Reslicing using ");
        let how = match resample_type_val {
            SAMPLE_TRILINEAR => "trilinear interpolation \n",
            SAMPLE_NEAREST => "nearest \n",
            SAMPLE_CUBIC => "cubic \n",
            SAMPLE_WEIGHTED => "weighted \n",
            SAMPLE_VOTE => "voting \n",
            _ => "",
        };
        let _ = write!(r.out, "{how}");
        let m2 = mri_resample(g, r.out, r.err, &mri, &mri_template, resample_type_val)?;
        let Some(m2) = m2 else {
            return Err(Fatal::Exit(1));
        };
        mri = m2;
    }

    // ----- invert contrast if necessary -----
    if o.invert_val >= 0.0 {
        let _ = writeln!(r.out, "inverting contrast...");
        let m2 = mri_invert_contrast(g, r.out, r.err, Pair::New(&mri), o.invert_val)?.into_new();
        let Some(m2) = m2 else {
            return Err(Fatal::Exit(1));
        };
        mri = m2;
    }

    // ----- reorder if necessary -----
    if o.shiftdim_flag != 0 {
        let _ = writeln!(r.out, "shifting dim ...");
        let d = o.shift_dim.unwrap_or([0; 3]);
        let Some(m2) = mri_shift_dim(g, r.out, r.err, &mri, d[0], d[1], d[2])? else {
            return Err(Fatal::Exit(1));
        };
        mri = m2;
    }
    if o.reorder_flag != 0 {
        let _ = writeln!(r.out, "reordering axes...");
        let v = o.reorder_vals.unwrap_or([0; 3]);
        let m2 = mri_reorder(g, r.out, r.err, &mri, None, v[0], v[1], v[2])?.into_new();
        let Some(m2) = m2 else {
            let _ = writeln!(r.err, "error reordering axes");
            return Err(Fatal::Exit(1));
        };
        mri = m2;
    }
    if o.reorder4_flag != 0 {
        let v = o.reorder4_vals.unwrap_or([0; 4]);
        let _ = writeln!(r.out, "reordering all axes...");
        let _ = writeln!(
            r.out,
            "reordering, values are {} {} {} {}",
            v[0], v[1], v[2], v[3]
        );
        let Some(m2) = mri_reorder4(g, r.out, r.err, &mri, v)? else {
            let _ = writeln!(r.err, "error reordering all axes");
            return Err(Fatal::Exit(1));
        };
        mri = m2;
    }

    // ----- store the gdf file stem -----
    mri.gdf_image_stem = o.gdf_image_stem.clone();

    // ----- catch the out info flag -----
    if o.out_info_flag != 0 {
        let _ = writeln!(r.out, "output structure:");
        dump(g, r, &mri)?;
    }
    if o.out_matrix_flag != 0 {
        let i_to_r = extract_i_to_r(g, r.out, r.err, &mri.vg)?;
        let _ = writeln!(r.out, "output ijk -> ras:");
        mprint(g, r, Some(&i_to_r));
    }
    if o.out_stats_flag != 0 {
        stats(g, r, &mri)?;
    }

    if o.erode_seg_flag != 0 {
        let n = o.n_erode_seg.unwrap_or(0);
        let _ = writeln!(r.out, "Eroding segmentation {n}");
        let m2 = mri_erode_segmentation(g, r.out, r.err, Pair::New(&mri), n, 0)?.into_new();
        let Some(m2) = m2 else {
            return Err(Fatal::Exit(1));
        };
        mri = m2;
    }
    if o.dil_seg_flag != 0 {
        let n = o.n_dil_seg.unwrap_or(0);
        let mut mask: Option<Mri> = None;
        if o.dil_seg_mask.is_empty() {
            let _ = writeln!(r.out, "Dilating segmentation {n}");
        } else {
            let _ = writeln!(r.out, "Dilating segmentation, mask {}", o.dil_seg_mask);
            mask = mri_read_volume(g, r.out, r.err, r.env, r.dir, &o.dil_seg_mask)?;
            if mask.is_none() {
                let _ = writeln!(r.out, "ERROR: could not read {}", o.dil_seg_mask);
                return Err(Fatal::Exit(1));
            }
        }
        let mut nchanges = 0;
        let m2 = mri_dilate_segmentation(
            g,
            r.out,
            r.err,
            Pair::New(&mri),
            n,
            mask.as_ref(),
            0,
            0.5,
            &mut nchanges,
        )?
        .into_new();
        let Some(m2) = m2 else {
            return Err(Fatal::Exit(1));
        };
        mri = m2;
    }

    if o.frame_flag != 0 {
        mri = keep_frames(g, r, &o, mri)?;
    }

    // ----- drop the last ndrop frames (drop before skip) -----
    if o.ndrop > 0 {
        let _ = writeln!(r.out, "Dropping last {} frames", o.ndrop);
        if mri.nframes <= o.ndrop {
            let _ = writeln!(
                r.out,
                "   ERROR: can't drop, volume only has {} frames",
                mri.nframes
            );
            return Err(Fatal::Exit(1));
        }
        let Some(m2) = fmri_ndrop(g, r.out, r.err, &mri, o.ndrop, None)?.into_new() else {
            return Err(Fatal::Exit(1));
        };
        mri = m2;
    }
    // ----- skip the first nskip frames -----
    if o.nskip > 0 {
        let _ = writeln!(r.out, "Skipping {} frames", o.nskip);
        if mri.nframes <= o.nskip {
            let _ = writeln!(
                r.out,
                "   ERROR: can't skip, volume only has {} frames",
                mri.nframes
            );
            return Err(Fatal::Exit(1));
        }
        let Some(m2) = fmri_nskip(g, r.out, r.err, &mri, o.nskip, None)?.into_new() else {
            return Err(Fatal::Exit(1));
        };
        mri = m2;
    }
    if o.subsample_flag != 0 {
        let [start, delta, end] = o.sub_samp.map(|v| v.unwrap_or(0));
        let _ = writeln!(
            r.out,
            "SubSample: Start = {start}  Delta = {delta}, End = {end}"
        );
        let Some(m2) = fmri_sub_sample(g, r.out, r.err, &mri, start, delta, end, None)?.into_new()
        else {
            return Err(Fatal::Exit(1));
        };
        mri = m2;
    }
    if o.cutends_flag != 0 {
        let _ = writeln!(r.out, "Cutting ends: n = {}", o.ncutends);
        let Some(m2) = mri_cut_end_slices(g, r.out, r.err, &mri, o.ncutends)? else {
            return Err(Fatal::Exit(1));
        };
        mri = m2;
    }

    // ----- crops -----
    if o.crop_flag != 0 {
        let x0 = o.crop_center[0] - o.crop_size[0] / 2;
        let y0 = o.crop_center[1] - o.crop_size[1] / 2;
        let z0 = o.crop_center[2] - o.crop_size[2] / 2;
        let t = mri_extract(
            g,
            r.out,
            r.err,
            &mut mri,
            None,
            x0,
            y0,
            z0,
            o.crop_size[0],
            o.crop_size[1],
            o.crop_size[2],
        )?
        .into_new();
        mri = need(g, r.err, t, "MRIextract")?;
    }

    let mut out_scale_factor = o.out_scale_factor;
    if o.rescale_voxel != 0 {
        let [c, rr, s] = o.rescale_voxel_crs.map(|v| v.unwrap_or(0));
        let _ = writeln!(r.out, "rescale voxel {c} {rr} {s}");
        let val = f64::from(mri_get_vox_val(g, r.err, &mri, c, rr, s, 0, 0)?);
        if val == 0.0 {
            let _ = writeln!(r.out, "Rescale voxel has a value of 0");
            return Err(Fatal::Exit(1));
        }
        out_scale_factor = div64(f64::from(out_scale_factor), val) as f32;
        let _ = writeln!(
            r.out,
            "Voxel val = {}, scaling by {}",
            cfmt::g(val, 6),
            gf(out_scale_factor)
        );
    }
    if !feq(out_scale_factor, 1.0) {
        let _ = writeln!(
            r.out,
            "scaling output intensities by {}",
            fwp(f64::from(out_scale_factor), 2, 3)
        );
        let t = mri_scalar_mul(g, r.out, r.err, Pair::Same(&mut mri), out_scale_factor)?;
        drop(t);
    }

    if o.sphinx_flag != 0 {
        let _ = writeln!(r.out, "Changing orientation information to Sphinx");
        hfs2sphinx(&mut mri.vg);
    }
    if let Some(a) = &o.auto_align {
        mri.auto_align = Some(a.clone());
    }

    // ----- modify the colour lookup table (--ctab) -----
    if o.colortablefile == "remove" {
        if mri.ct.is_some() {
            let _ = writeln!(r.out, "removing color lookup table");
            mri.ct = None;
        }
    } else if !o.colortablefile.is_empty() {
        let _ = writeln!(r.out, "embedding color lookup table");
        mri.ct = ctab_read_ascii(g, r.out, r.err, r.dir, &o.colortablefile);
        if mri.ct.is_none() {
            let m = format!("could not read lookup table from {}", o.colortablefile);
            log::fatal(g, r.err, 1, &m)?;
            return Err(Fatal::Exit(1));
        }
    } else if let Some(ct) = ctembedded {
        mri.ct = Some(ct);
    }

    // ------ finally, write the output -----
    if o.out_stat_table_flag != 0 {
        let _ = writeln!(
            r.out,
            "Writing as Stats-Table to {out_name} using template {out_like_name}"
        );
        let mut io = Io {
            out: &mut *r.out,
            err: &mut *r.err,
            env: &*r.env,
            dir: r.dir,
        };
        let st = init_stat_table_from_mri(g, &mut io, &mri, o.out_like_name.as_deref())?;
        let Some(st) = st else {
            return no_table(g, r, &out_name);
        };
        write_stat_table(g, &mut io, &out_name, &st)?;
        let _ = writeln!(r.out, "done");
        return Err(Fatal::Exit(0));
    }

    if o.ascii_flag != 0 {
        return write_ascii(g, r, &o, &mri, &out_name);
    }

    if o.no_write_flag == 0 {
        mri.orig_ras2vox = m_orig_ras2vox.clone();
        if let Some(m) = &m_orig_ras2vox {
            let _ = writeln!(r.out, "orig ras2vox:");
            mprint(g, r, Some(m));
        }
        if o.split_frames == 0 {
            let _ = writeln!(r.out, "writing to {out_name}...");
            if o.force_out_type_flag != 0 {
                let e = mri_write_type(
                    g,
                    r.out,
                    r.err,
                    r.env,
                    r.dir,
                    &mut mri,
                    &out_name,
                    o.out_volume_type,
                    MGZ_INTENT_UNKNOWN,
                )?;
                if e != NO_ERROR {
                    let _ = writeln!(
                        r.out,
                        "ERROR: failure writing {out_name} as volume type {}",
                        o.out_volume_type
                    );
                    return Err(Fatal::Exit(1));
                }
            } else {
                let mut several: [Mri; 0] = [];
                let e = mri_write(
                    g,
                    r.out,
                    r.err,
                    r.env,
                    r.dir,
                    &mut mri,
                    &out_name,
                    Some(&mut several),
                    MGZ_INTENT_UNKNOWN,
                )?;
                if e != NO_ERROR {
                    let _ = writeln!(r.out, "ERROR: failure writing {out_name}");
                    return Err(Fatal::Exit(1));
                }
            }
        } else {
            split_frames(g, r, &mri, &out_name)?;
        }
    }

    drop(mri_template);

    if o.first_dicom != 0 {
        // only a DICOM read fills the name (3.7-14)
        let first = &o.dicom.first_dicom_file;
        if first.is_empty() {
            let _ = writeln!(r.out, "ERROR: there is no first dicom file");
            return Err(Fatal::Exit(1));
        }
        let name = o.first_dicom_file.clone().unwrap_or_default();
        let _ = writeln!(
            r.out,
            "Writing FirstDicomFile {} to {name}",
            String::from_utf8_lossy(first)
        );
        match larmorx_freesurfer_io::cfile::CFile::open(r.dir, &name, "w") {
            Ok(mut fp) => {
                fp.put_bytes(first);
                fp.put_bytes(b"\n");
                fp.close();
            }
            Err(e) => {
                g.errno = e;
                let _ = writeln!(r.out, "ERROR: opening {name}");
                return Err(Fatal::Exit(1));
            }
        }
    }
    Err(Fatal::Exit(0))
}

/// `FEQUAL(a, b)`: `fabs(a - b) < 1e-10` (in `double`).
fn feq(a: f32, b: f64) -> bool {
    (f64::from(a) - b).abs() < 1e-10
}

/// The transform types `main` reads with `LTAreadEx`.
fn is_linear(t: i32) -> bool {
    t == MNI_TRANSFORM_TYPE || t == TRANSFORM_ARRAY_TYPE || t == REGISTER_DAT || t == FSLREG_TYPE
}

/// `MRIallocHeader(<mri's extent, type, frames>)` with `MRIcopyHeader(mri, ...)`.
fn header_like(g: &mut Globals, r: &mut Run, mri: &Mri) -> Result<Mri, Fatal> {
    let mut t = mri_alloc_header(
        g,
        r.out,
        r.err,
        mri.width,
        mri.height,
        mri.depth,
        mri.mtype,
        mri.nframes,
    )?;
    let _ = mri_copy_header(g, r.out, r.err, mri, Some(&mut t))?;
    Ok(t)
}

/// `MRIdump(mri, stdout)`.
fn dump(g: &mut Globals, r: &mut Run, mri: &Mri) -> Result<(), Fatal> {
    let mut buf: Vec<u8> = Vec::new();
    mri_dump(g, r.out, r.err, mri, &mut buf)?;
    let _ = r.out.write_all(&buf);
    Ok(())
}

/// `MRIprintStats(mri, stdout)`.
fn stats(g: &mut Globals, r: &mut Run, mri: &Mri) -> Result<(), Fatal> {
    mri_print_stats(g, r.out, r.err, mri)?;
    Ok(())
}

/// `MRIreadHeader(out_like_name, MRI_VOLUME_TYPE_UNKNOWN)` of a volume `main` resamples into
/// (FS-1047: FreeSurfer uses the header it could not read).
fn read_like(g: &mut Globals, r: &mut Run, name: &str) -> Result<Mri, Fatal> {
    let tmp = mri_read_header(g, r.out, r.err, r.env, r.dir, name, MRI_VOLUME_TYPE_UNKNOWN)?;
    match tmp {
        Some(t) => Ok(t),
        None => {
            g.notices.add(
                &FS_1047,
                format!("mri_convert --like {name}: a volume that cannot be read, which FreeSurfer uses (a crash)"),
            );
            let _ = writeln!(
                r.err,
                "mri_convert: the --like volume {name} could not be read (FreeSurfer crashes here)"
            );
            Err(Fatal::Exit(1))
        }
    }
}

/// `--left-right-reverse`: the geometry reversed along the axes that point left or right (their
/// direction cosines negated), the centre moved so that reversed voxels would register.
fn left_right_reverse(g: &mut Globals, r: &mut Run, mri: &mut Mri) -> Result<(), Fatal> {
    let _ = write!(
        r.out,
        "WARNING: applying left-right reversal to the input geometry\nwithout reversing the pixel data. This will likely make \nthe volume geometry WRONG, so make sure you know what you  \nare doing.\n"
    );
    let _ = writeln!(
        r.out,
        "  CRAS Before: {} {} {}",
        gf(mri.c_r),
        gf(mri.c_a),
        gf(mri.c_s)
    );
    let t = xfm_crs2xyz(g, r.out, r.err, &mri.vg, 0, false)?;
    let mut vmid = Matrix::real(4, 1);
    vmid.set_relt(4, 1, 1.0);
    let (_, ostr) = dircos_to_orientation_string(&mri.vg);
    let lr = |c: u8| c == b'L' || c == b'R';
    let mut cras: Option<Matrix> = None;
    let (w, h, d) = (
        f64::from(mri.width),
        f64::from(mri.height),
        f64::from(mri.depth),
    );
    let mid = |v: f64, minus: bool| (if minus { v / 2.0 - 1.0 } else { v / 2.0 }) as f32;
    for (k, name) in [(0, "columns"), (1, "rows"), (2, "slices")] {
        if !lr(ostr[k]) {
            continue;
        }
        let _ = writeln!(r.out, "  Reversing geometry for the {name}");
        vmid.set_relt(1, 1, mid(w, k == 0));
        vmid.set_relt(2, 1, mid(h, k == 1));
        vmid.set_relt(3, 1, mid(d, k == 2));
        cras = matrix_multiply(g, r.out, r.err, Some(&t), Some(&vmid), None)?.into_new();
        // `x *= -1.0`: the official build flips the sign bit (a NaN's too)
        let v = &mut mri.vg;
        match k {
            0 => {
                v.x_r = -v.x_r;
                v.x_a = -v.x_a;
                v.x_s = -v.x_s;
            }
            1 => {
                v.y_r = -v.y_r;
                v.y_a = -v.y_a;
                v.y_s = -v.y_s;
            }
            _ => {
                v.z_r = -v.z_r;
                v.z_a = -v.z_a;
                v.z_s = -v.z_s;
            }
        }
    }
    let Some(cras) = cras else {
        g.notices.add(
            &FS_1043,
            format!(
                "mri_convert --left-right-reverse: no axis of the input points left or right (orientation {}), where FreeSurfer reads a centre it never computed (a crash)",
                String::from_utf8_lossy(&ostr)
            ),
        );
        let _ = writeln!(
            r.err,
            "mri_convert: --left-right-reverse: no axis of the input points left or right (FreeSurfer crashes here)"
        );
        return Err(Fatal::Exit(1));
    };
    mri.c_r = cras.relt(1, 1);
    mri.c_a = cras.relt(2, 1);
    mri.c_s = cras.relt(3, 1);
    let _ = writeln!(
        r.out,
        "  CRAS After {} {} {}",
        gf(mri.c_r),
        gf(mri.c_a),
        gf(mri.c_s)
    );
    Ok(())
}

/// `--left-right-mirror` / `--left-right-keep`: one half of the volume along its left-right axes
/// mirrored into the other (keep: the other half set to 0).
fn left_right_mirror(g: &mut Globals, r: &mut Run, o: &Options, mri: Mri) -> Result<Mri, Fatal> {
    let hemi = o.left_right_mirror_hemi.clone().unwrap_or_default();
    let _ = write!(
        r.out,
        "WARNING: Mirroring {hemi} half into the other half,\nor masking one half of the image is only meaningful if\nthe image is upright and centerd, see make_upright.\n"
    );
    let (_, ostr) = dircos_to_orientation_string(&mri.vg);
    let _ = writeln!(
        r.out,
        "  Orientation string: {}",
        String::from_utf8_lossy(&ostr)
    );
    let mut mri2 = copy_of(g, r, &mri)?;
    let hemi0 = hemi
        .as_bytes()
        .first()
        .copied()
        .unwrap_or(0)
        .to_ascii_uppercase();
    let mirror = o.left_right_mirror_flag != 0;
    let lr = |c: u8| c == b'L' || c == b'R';
    let mut v = 0.0f32;
    if lr(ostr[0]) {
        let _ = writeln!(r.out, "  Mirror or keep pixels for the columns");
        for c in 0..mri.width / 2 {
            let c1 = if ostr[0] == hemi0 {
                mri.width - c - 1
            } else {
                c
            };
            let c2 = mri.width - c1 - 1;
            for rr in 0..mri.height {
                for s in 0..mri.depth {
                    for f in 0..mri.nframes {
                        if mirror {
                            v = mri_get_vox_val(g, r.err, &mri, c1, rr, s, f, 0)?;
                        }
                        mri_set_vox_val(g, r.err, &mut mri2, c2, rr, s, f, v, 0)?;
                    }
                }
            }
        }
    }
    if lr(ostr[1]) {
        let _ = writeln!(r.out, "  Mirror or keep pixels for the rows");
        for c in 0..mri.width {
            for rr in 0..mri.height / 2 {
                let r1 = if ostr[1] == hemi0 {
                    mri.height - rr - 1
                } else {
                    rr
                };
                let r2 = mri.height - r1 - 1;
                for s in 0..mri.depth {
                    for f in 0..mri.nframes {
                        if mirror {
                            v = mri_get_vox_val(g, r.err, &mri, c, r1, s, f, 0)?;
                        }
                        mri_set_vox_val(g, r.err, &mut mri2, c, r2, s, f, v, 0)?;
                    }
                }
            }
        }
    }
    if lr(ostr[2]) {
        let _ = writeln!(r.out, "  Mirror or keep pixels for the slices");
        for c in 0..mri.width {
            for rr in 0..mri.height {
                for s in 0..mri.depth / 2 {
                    let s1 = if ostr[2] == hemi0 {
                        mri.depth - s - 1
                    } else {
                        s
                    };
                    let s2 = mri.depth - s1 - 1;
                    for f in 0..mri.nframes {
                        if mirror {
                            v = mri_get_vox_val(g, r.err, &mri, c, rr, s1, f, 0)?;
                        }
                        mri_set_vox_val(g, r.err, &mut mri2, c, rr, s2, f, v, 0)?;
                    }
                }
            }
        }
    }
    Ok(mri2)
}

/// `--invert_transform` of a linear transform: its matrix inverted, its source and destination
/// geometries swapped (a source without one taken from `<transform's folder>/../orig` when that
/// volume can be read).
fn invert_lta(
    g: &mut Globals,
    r: &mut Run,
    lta: &mut Lta,
    transform_fname: &str,
) -> Result<(), Fatal> {
    let inv = matrix_inverse(g, r.out, r.err, lta.xforms[0].m_l.as_ref(), None)?.into_new();
    let Some(inv) = inv else {
        let _ = writeln!(r.err, "ERROR: inverting transform");
        let m = lta.xforms[0].m_l.clone();
        mprint(g, r, m.as_ref());
        return Err(Fatal::Exit(1));
    };
    lta.xforms[0].m_l = Some(inv);
    // the source and destination swapped: they decide the result's c_ras in LTAtransform
    if lta.xforms[0].src.valid == 0 {
        let _ = writeln!(
            r.err,
            "INFO: Trying to get the source volume information from the transform name"
        );
        let mut buf = match pathsep::rfind_sep_str(transform_fname) {
            Some(p) => transform_fname[..=p].to_owned(),
            None => "./".to_owned(),
        };
        buf.push_str("../orig");
        if transform_fname.len() >= 512 || buf.len() >= 512 {
            g.notices.add(
                &FS_1046,
                format!(
                    "mri_convert --invert_transform: a source volume name of {} bytes built in FreeSurfer's 512-byte buffer (FreeSurfer writes past it)",
                    buf.len().max(transform_fname.len())
                ),
            );
        }
        let orig = mri_read_header(g, r.out, r.err, r.env, r.dir, &buf, MRI_VOLUME_TYPE_UNKNOWN)?;
        match orig {
            Some(orig) => {
                get_vol_geom(&orig.vg, &mut lta.xforms[0].src);
                let _ = writeln!(
                    r.err,
                    "INFO: Succeeded in retrieving the source volume info."
                );
            }
            None => {
                let _ = write!(
                    r.out,
                    "INFO: Failed to find {buf} as a source volume.  \n      The inverse c_(ras) may not be valid.\n"
                );
            }
        }
    }
    let lt = &mut lta.xforms[0];
    let mut vgtmp = VolGeom::default();
    vgtmp.assign(g, &lt.dst);
    let src = lt.src.clone();
    lt.dst.assign(g, &src);
    lt.src.assign(g, &vgtmp);
    Ok(())
}

/// FS-1048: the inverted morph's result written to `t.mgz` in the working folder (a debugging
/// line FreeSurfer left in): by default not written.
fn write_t_mgz(g: &mut Globals, r: &mut Run, m: &mut Mri) -> Result<(), Fatal> {
    g.notices.add(
        &FS_1048,
        "mri_convert: an inverted morph writes its result to t.mgz in the working folder",
    );
    if g.notices.reproduce() {
        let _ = mri_write(
            g,
            r.out,
            r.err,
            r.env,
            r.dir,
            m,
            "t.mgz",
            None,
            MGZ_INTENT_UNKNOWN,
        )?;
    }
    Ok(())
}

/// `MRIhalfCosBias(mri, alpha, mri)`: `MRIcopy` of a volume onto itself does nothing, then each
/// voxel of slice `s` times `w = (alpha cos(pi s / (depth - 1)) + (2 - alpha)) / 2` (`double`,
/// glibc's `cos`, the official build's operand orders: fs.volume's `mri_half_cos_bias`).
fn half_cos_bias_in_place(
    g: &mut Globals,
    r: &mut Run,
    d: &mut Mri,
    alpha: f64,
) -> Result<(), Fatal> {
    let widen = |x: f32| {
        if x.is_nan() {
            f64::from(f32::from_bits(x.to_bits() | 0x0040_0000))
        } else {
            f64::from(x)
        }
    };
    let libm = Libm::new(g.mode());
    for s in 0..d.vg.depth {
        let arg = div64(
            mul64(std::f64::consts::PI, f64::from(s)),
            sub64(f64::from(d.vg.depth), 1.0),
        );
        let cs = libm.cos(arg);
        let w = mul64(add64(mul64(cs, alpha), sub64(2.0, alpha)), 0.5);
        for c in 0..d.vg.width {
            for rr in 0..d.vg.height {
                for f in 0..d.nframes {
                    let v = widen(mri_get_vox_val(g, r.err, d, c, rr, s, f, 0)?);
                    mri_set_vox_val(g, r.err, d, c, rr, s, f, mul64(v, w) as f32, 0)?;
                }
            }
        }
    }
    note_glibc07(g, &libm, "cos");
    Ok(())
}

/// `--frame` / `--mid-frame`: the frames kept, in the order given.
fn keep_frames(g: &mut Globals, r: &mut Run, o: &Options, mri: Mri) -> Result<Mri, Fatal> {
    let mut frames = o.frames.clone();
    let mut nframes = o.nframes;
    if o.mid_frame_flag != 0 {
        nframes = 1;
        let mid = nint(g, f64::from(mri.nframes / 2));
        if frames.is_empty() {
            frames.push(mid);
        } else {
            frames[0] = mid;
        }
    }
    let at = |k: i32| frames.get(k as usize).copied().unwrap_or(0);
    if nframes == 1 {
        let _ = writeln!(r.out, "keeping frame {}", at(0));
    } else {
        let _ = write!(r.out, "keeping frames");
        for f in 0..nframes {
            let _ = write!(r.out, " {}", at(f));
        }
        let _ = writeln!(r.out);
    }
    let mut mri2 = mri_alloc_sequence(
        g, r.out, r.err, mri.width, mri.height, mri.depth, mri.mtype, nframes,
    )?;
    let _ = mri_copy_header(g, r.out, r.err, &mri, Some(&mut mri2))?;
    mri_copy_pulse_parameters(&mri, &mut mri2);
    for f in 0..nframes {
        let src = at(f);
        if src < 0 || src >= mri.nframes {
            let _ = writeln!(
                r.out,
                "   ERROR: valid frame numbers are between 0 and {}",
                mri.nframes - 1
            );
            return Err(Fatal::Exit(1));
        }
        for s in 0..mri2.depth {
            for rr in 0..mri2.height {
                for c in 0..mri2.width {
                    let v = mri_get_vox_val(g, r.err, &mri, c, rr, s, src, 0)?;
                    mri_set_vox_val(g, r.err, &mut mri2, c, rr, s, f, v, 0)?;
                }
            }
        }
    }
    Ok(mri2)
}

/// FS-1050: `--out_stats_table` when the table could not be made: FreeSurfer creates the file,
/// then prints from the table it does not have (a crash); both modes create it and end.
fn no_table(g: &mut Globals, r: &mut Run, out_name: &str) -> Result<(), Fatal> {
    g.notices.add(
        &FS_1050,
        format!("mri_convert --out_stats_table {out_name}: no table could be made from the volume, which FreeSurfer then prints (a crash)"),
    );
    match larmorx_freesurfer_io::cfile::CFile::open(r.dir, out_name, "w") {
        Ok(fp) => {
            fp.close();
        }
        Err(e) => {
            g.errno = e;
            let _ = writeln!(r.out, "ERROR: cannot open {out_name}");
            return Err(Fatal::Exit(1));
        }
    }
    let _ = writeln!(
        r.err,
        "mri_convert: --out_stats_table: no table could be made from the volume (FreeSurfer crashes here)"
    );
    Err(Fatal::Exit(1))
}

/// `--ascii` (1: one value a line), `--ascii+crsf` (2: column, row, slice, frame and the value),
/// `--ascii-fcol` (3: a voxel's frames on one line), each value `%lf`; `done`.
fn write_ascii(
    g: &mut Globals,
    r: &mut Run,
    o: &Options,
    mri: &Mri,
    out_name: &str,
) -> Result<(), Fatal> {
    let _ = writeln!(r.out, "Writing as ASCII to {out_name}");
    let mut fp = match larmorx_freesurfer_io::cfile::CFile::open(r.dir, out_name, "w") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            g.notices.add(
                &FS_1049,
                format!("mri_convert --ascii {out_name}: a file that cannot be created, which FreeSurfer then writes (a crash)"),
            );
            let _ = writeln!(
                r.err,
                "mri_convert: {out_name} cannot be created (FreeSurfer crashes here)"
            );
            return Err(Fatal::Exit(1));
        }
    };
    let lf = |v: f32| cfmt::f(f64::from(v), 6);
    if o.ascii_flag == 1 || o.ascii_flag == 2 {
        for f in 0..mri.nframes {
            for s in 0..mri.depth {
                for rr in 0..mri.height {
                    for c in 0..mri.width {
                        if o.ascii_flag == 1 {
                            let v = mri_get_vox_val(g, r.err, mri, c, rr, s, f, 0)?;
                            fp.put_bytes(format!("{}\n", lf(v)).as_bytes());
                        }
                        if o.ascii_flag == 2 {
                            let v = mri_get_vox_val(g, r.err, mri, c, rr, s, f, 0)?;
                            let line = format!("{c:3} {rr:3} {s:3} {f:3} {}\n", lf(v));
                            fp.put_bytes(line.as_bytes());
                        }
                    }
                }
            }
        }
    }
    if o.ascii_flag == 3 {
        for s in 0..mri.depth {
            for rr in 0..mri.height {
                for c in 0..mri.width {
                    for f in 0..mri.nframes {
                        let v = mri_get_vox_val(g, r.err, mri, c, rr, s, f, 0)?;
                        fp.put_bytes(lf(v).as_bytes());
                        if f != mri.nframes - 1 {
                            fp.put_bytes(b" ");
                        }
                    }
                    fp.put_bytes(b"\n");
                }
            }
        }
    }
    fp.close();
    let _ = writeln!(r.out, "done");
    Err(Fatal::Exit(0))
}

/// `--split`: each frame written to `<stem><%04d>.<ext>`.
fn split_frames(g: &mut Globals, r: &mut Run, mri: &Mri, out_name: &str) -> Result<(), Fatal> {
    let stem = id_stem_from_name(out_name);
    let ext = id_extension_from_name(g, out_name);
    let s = stem.as_deref().unwrap_or("(null)");
    let e = ext.as_deref().unwrap_or("(null)");
    let _ = writeln!(r.out, "Splitting frames, stem = {s}, ext = {e}");
    let mut mri2: Option<Mri> = None;
    for i in 0..mri.nframes {
        match mri2.as_mut() {
            None => {
                let t = mri_copy_frame(g, r.out, r.err, Pair::New(mri), i, 0)?.into_new();
                mri2 = Some(need(g, r.err, t, "MRIcopyFrame")?);
            }
            Some(m2) => {
                let t = mri_copy_frame(g, r.out, r.err, Pair::Given(mri, m2), i, 0)?;
                if t.is_null() {
                    need::<Mri>(g, r.err, None, "MRIcopyFrame")?;
                }
            }
        }
        let m2 = mri2.as_mut().expect("the frame volume");
        let name = format!("{s}{i:04}.{e}");
        if name.len() >= STRLEN {
            g.notices.add(
                &FS_1051,
                format!(
                    "mri_convert --split: a frame's name of {} bytes built in FreeSurfer's 1,024-byte buffer (FreeSurfer writes past it)",
                    name.len()
                ),
            );
        }
        let _ = writeln!(r.out, "{i:2} {name}");
        let e2 = mri_write(
            g,
            r.out,
            r.err,
            r.env,
            r.dir,
            m2,
            &name,
            None,
            MGZ_INTENT_UNKNOWN,
        )?;
        if e2 != NO_ERROR {
            let _ = writeln!(r.out, "ERROR: failure writing {name}");
            return Err(Fatal::Exit(1));
        }
    }
    Ok(())
}

/// `applyGaussianFilter(inVol, voxsize, nthreads)` (`--antialias --voxsize`): each axis whose
/// output voxel is larger than its input voxel smoothed with the Gaussian of standard deviation
/// `fwhm / sqrt(log(256))`, `fwhm = sigma * sqrt(8 log 2)`, `sigma = R log(5) / pi` (`R` the
/// output voxel size, in voxels); the others not smoothed (0).
fn apply_gaussian_filter(
    g: &mut Globals,
    r: &mut Run,
    vol: &mut Mri,
    voxsize: [f32; 3],
) -> Result<(), Fatal> {
    let _ = writeln!(r.out, "Applying Gaussian Filter ...");
    let mut std = [0.0f32; 3];
    for n in 0..3 {
        let insize = match n {
            0 => vol.xsize,
            1 => vol.ysize,
            _ => vol.zsize,
        };
        std[n] = 0.0;
        if voxsize[n] > insize {
            let rr = voxsize[n];
            // the official build: R * logf(5) in float (folded), over M_PI in double
            let sigma = div64(f64::from(mul(rr, LOGF_5)), std::f64::consts::PI) as f32;
            // sqrt(8 * log(2)) * sigma (the product's operands as the official build orders them)
            let fwhm = mul64(SQRT_8_LOG_2, f64::from(sigma)) as f32;
            std[n] = div64(f64::from(fwhm), SQRT_LOG_256) as f32;
            let _ = writeln!(
                r.out,
                "dimension #{}: insize = {}, outsize = {}, sigma = {}, fwhm = {}, gstd = {}",
                n + 1,
                ff(f64::from(insize)),
                ff(f64::from(voxsize[n])),
                gf(sigma),
                gf(fwhm),
                gf(std[n])
            );
        }
    }
    let t = mri_gaussian_smooth_ni(
        g,
        r.out,
        r.err,
        Pair::Same(vol),
        f64::from(std[0]),
        f64::from(std[1]),
        f64::from(std[2]),
        &Smni::default(),
    )?;
    drop(t);
    Ok(())
}

/// `log(F)` of `F = 5.0f` as the official build computes it: C++'s `float` overload, folded by
/// the compiler to the correctly rounded `logf(5)` (`0x3fce0210`).
const LOGF_5: f32 = f32::from_bits(0x3fce_0210);
/// `sqrt(8 * log(2))`: the product folded by the compiler (`8 * log(2)` correctly rounded,
/// 5.545177444479562), its square root at run time (correctly rounded).
const SQRT_8_LOG_2: f64 = 2.3548200450309493;
/// `sqrt(log(256.0))`: glibc's `log(256.0)` at run time (5.545177444479562, correctly rounded),
/// then its square root.
const SQRT_LOG_256: f64 = 2.3548200450309493;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_constants_of_the_filter() {
        // the correctly rounded logf(5), 8 log(2) and log(256) (one double), their square roots
        assert_eq!(f64::from(LOGF_5), 1.6094379425048828);
        assert_eq!(
            5.545177444479562f64.sqrt().to_bits(),
            SQRT_8_LOG_2.to_bits()
        );
        assert_eq!(
            5.545177444479562f64.sqrt().to_bits(),
            SQRT_LOG_256.to_bits()
        );
    }
}
