// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_vol2vol`'s `main` (`mri_vol2vol.cpp` 640-1310): the program's entry point [`run`]
//! (registered as `mri_vol2vol` in the crate's `PROGRAMS`) and [`main`].
//!
//! The order of `main`: the version options (`handleVersionOption`, which here leaves the
//! argument count as it was: FS-07); `FREESURFER_HOME` must be set (`ERROR: FREESURFER_HOME
//! undefined.`, status 1); no argument: the usage, status 1; the options and their checks
//! ([`super::options`]); the noise seed (`PDFtodSeed` unless `--seed`; FS-29) and `srand48`; the
//! settings (`dump_options`). Then one of the paths:
//! - `--crop <scale>`: the moving volume cut to the box of its voxels above 0 (`MRIboundingBox`,
//!   `MRIcrop`), resampled by nearest neighbour onto a grid `scale` times finer (`MRIvol2Vol`),
//!   written; `#VMPC# mri_vol2vol VmPeak <kB>`.
//! - `--tal`: the registration composed with the inverse of the subject's `talairach.xfm`
//!   (`DevolveXFM`) and of `$FREESURFER_HOME/average/mni305.cor.subfov<res>.reg`.
//! - The input (the moving volume, or with `--inv` the target) read whole and the other one's
//!   header (or the LTA's geometry, or with `--downsample` a coarser grid of the input); `--soap`
//!   (`MRIsoapBubble` of the moving volume with the control points, written); `--synth` (noise);
//!   the registration from the scanner coordinates (`--regheader`, `--xfm`: `MRItkRegMtx`) or FSL's
//!   (`MRIfsl2TkReg`, the second matrix subtracted); `ERROR: no registration (R) is specified`;
//!   `--rot`, `--trans`, `--shear` (each `R = M R`, printed); `--reg-final`; `--inv` (the matrix
//!   inverted); the final matrix printed.
//! - `--no-resample`: the input's voxel-to-RAS matrix becomes `Stemp inv(Ttemp) inv(R) Tin`
//!   (`MRIsetVoxelToRasXform`), `--slice-crop`, `--slice-reverse`, `--slice-bias`, written.
//! - Otherwise the voxel-to-voxel matrix `inv(Tin) R Ttemp` printed; the output (the template's
//!   geometry, `--precision`'s type, the input's pulse parameters unless `--mr`): the trilinear
//!   kernel (`--kernel`), the displacement (`--delta`), the fill (`--fill-average`,
//!   `--fill-conserve`, `--downsample`: `MRIvol2VolFill`), or the resampling (`MRIvol2Vol`; with a
//!   voxel shift map or `--new`, `MRIvol2VolVSM`); `--slice-crop`, `--mul`; the colour table
//!   (`--ctab`, else the input's); written; `--save-reg` (`<out>.lta` and `<out>.reg`, the identity
//!   or with `--tal` the subfov registration); `mri_vol2vol done`.
//! - Part 2 (relay 54): `--disp` after the final matrix (the affine displacement, written; without
//!   `--o` the program ends there); the morphs in place of the resampling (`morph`: `--morph`,
//!   `--inv-morph`, `--m3z`, `--filter`); `--cost` after the resampling (`seg_reg_cost_line`).
//!   `--spm-warp`, `--gcam` and `--map-point` run while the options are read
//!   ([`super::standalone`]).
//!
//! Defects (`larmorx_core::defects`; the mechanisms in `docs/findings/freesurfer-mri-vol2vol.md`):
//! - **FS-1177**: `--crop` without an LTA or `--downsample` stops with `ERROR: No target volume
//!   supplied.` though it makes its own target: by default it crops (rule A).
//! - **FS-1178**: names built into FreeSurfer's fixed buffers (2,000 bytes for the target and
//!   registration names, 1,000 for `--save-reg`'s): a longer one aborts the official build
//!   (`*** buffer overflow detected ***: terminated`, status 134); by default the whole name.
//! - **FS-1179**: `--downsample` with a factor of 0 or below for the second or third axis: FreeSurfer
//!   divides by 0 (status 136) or makes a grid of negative size; by default an error (rule B).
//! - **FS-1180**: [`super::options::check_options`].
//! - **FS-1181**: `--crop` cuts one voxel more than the box on each axis, past the volume when the
//!   box reaches its last voxel (FreeSurfer then crashes): by default the box (rule A).
//! - **FS-1182**: `--crop` with a scale that leaves no voxel (0 or below, or so small that a size
//!   rounds to 0): FreeSurfer writes an empty volume (status 0) or stops at the allocation; by
//!   default an error (rule B).
//! - **FS-1186**, **FS-1187**, **FS-1193**: `seg_reg_cost_line`. **FS-1191**: `morph`.

use std::io::Write;

use larmorx_core::cfmt;
use larmorx_core::defects::{
    FS_1178, FS_1179, FS_1181, FS_1182, FS_1186, FS_1187, FS_1191, FS_1193,
};
use larmorx_freesurfer_core::affine::float_to_int;
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::diag::diag_init;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal, error_exit};
use larmorx_freesurfer_core::matrix::Matrix;
use larmorx_freesurfer_core::matrix_ops::{matrix_inverse, matrix_subtract};
use larmorx_freesurfer_core::pdf::pdf_tod_seed;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::{Environment, Settings};
use larmorx_freesurfer_core::sse::{div64, mul64};
use larmorx_freesurfer_core::system;
use larmorx_freesurfer_core::utils::{get_vm_peak, nint};
use larmorx_freesurfer_core::version::handle_version_option_whole;
use larmorx_freesurfer_filter::mode::mri_mode_filter;
use larmorx_freesurfer_filter::norm::{CONTROL_MARKED, mri_soap_bubble};
use larmorx_freesurfer_gca::gca::gca_alloc_max;
use larmorx_freesurfer_gca::gcam::{gcam_invert, gcam_read};
use larmorx_freesurfer_gca::ltamatrix::{SegRegCosts, mri_affine_displacement, seg_reg_cost};
use larmorx_freesurfer_gca::morph::{gcam_morph_from_atlas_gca, gcam_morph_to_atlas};
use larmorx_freesurfer_gca::transform::{GcaLtaIo, Transform, Xform, gcam_apply_transform};
use larmorx_freesurfer_io::cfile::CFile;
use larmorx_freesurfer_io::colortab::ctab_read_ascii;
use larmorx_freesurfer_io::identify::id_name_from_stem;
use larmorx_freesurfer_io::mriio::mri_write;
use larmorx_freesurfer_io::registerio::{FLT2INT_ROUND, regio_write_register};
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::ctab::ctab_deep_copy;
use larmorx_freesurfer_mri::lta::{LINEAR_VOX_TO_VOX, REGISTER_DAT, lta_alloc};
use larmorx_freesurfer_mri::mri::{
    MGZ_INTENT_UNKNOWN, MRI_FLOAT, Mri, SAMPLE_NEAREST, mri_alloc, mri_alloc_header,
    mri_copy_header, mri_copy_pulse_parameters, mri_copy_vol_geom_to_mri, use_vol_geom_to_mri,
};
use larmorx_freesurfer_mri::tkreg::{fsl2tkreg, tkreg_mtx, xfm_crs2xyz_tkreg};
use larmorx_freesurfer_mri::vol_geom::{VolGeom, xfm_crs2xyz};
use larmorx_freesurfer_mri::voxel::mri_bounding_box;
use larmorx_freesurfer_volume::arith::{Pair, mri_binarize, mri_clone, mri_clone_by_space};
use larmorx_freesurfer_volume::frames::mri_randn;
use larmorx_freesurfer_volume::ltaapply::{devolve_xfm, lta_modify_src_dst_geom};
use larmorx_freesurfer_volume::ltaops::lta_change_type;
use larmorx_freesurfer_volume::ltawrite::lta_write;
use larmorx_freesurfer_volume::orient::mri_set_voxel_to_ras_xform;
use larmorx_freesurfer_volume::vol2vol::{
    mri_vol2vol, mri_vol2vol_delta, mri_vol2vol_fill, mri_vol2vol_tl_kernel, mri_vol2vol_vsm_env,
    transform_reg_dat2_lta,
};
use larmorx_freesurfer_volume::voxops::{
    mri_const_op, mri_crop, mri_half_cos_bias, mri_reverse_slices,
};

use super::options::{
    Options, check_options, dump_options, identity, load_rtal, or_null, parse_commandline,
    print_matrix, read_header, read_volume, usage_exit,
};

/// The program's name as `argv[0]` gives it (the scripts and sMRIPrep run it by name).
pub const NAME: &str = "mri_vol2vol";

/// The program's streams, environment (its own copy: the options set variables) and working
/// directory.
pub struct Run<'a, 'b> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
    pub env: Environment,
    pub dir: Option<&'b std::path::Path>,
}

/// `mri_vol2vol`'s entry point: its original arguments (without the program's name), the
/// settings, the two streams; the exit status. The notices of the run follow on standard error
/// (`larmorx: ...`).
pub fn run(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mut g = Globals::new(settings.mode);
    // the process's own standard error, as the C library's perror sees it
    g.stderr.read_write = system::stderr_read_write();
    let status = {
        let mut terr = log::TrackedErr::new(&g, &mut *err);
        let mut r = Run {
            out: &mut *out,
            err: &mut terr,
            env: settings.env.clone(),
            dir: settings.dir.as_deref(),
        };
        match main(&mut g, &mut r, args) {
            Ok(()) => 0,
            Err(f) => f.status(),
        }
    };
    let _ = out.flush();
    g.notices.print(err);
    status
}

/// FS-1178: a name of `room` bytes or more (with its NUL past `room`) built with `sprintf` into a
/// buffer of `room` bytes: the official build aborts (`reproduce`: noted here, the abort where
/// the name is built); by default the whole name.
pub fn fixed_buffer(g: &mut Globals, text: &str, room: usize, what: &str) -> bool {
    if text.len() < room {
        return false;
    }
    g.notices.add(
        &FS_1178,
        format!(
            "mri_vol2vol: {what} of {} bytes used whole (FreeSurfer's buffer holds {}: the official build aborts)",
            text.len(),
            room - 1
        ),
    );
    g.notices.reproduce()
}

/// The fortified `sprintf`'s abort (FS-1178, `reproduce`).
pub fn overflow_abort(r: &mut Run) -> Fatal {
    let _ = r
        .err
        .write_all(b"*** buffer overflow detected ***: terminated\n");
    Fatal::Exit(134)
}

/// `MatrixMultiply(a, b, NULL)`: the new product, `None` for `NULL` (a `NULL` input ends the
/// program as `MatrixMultiply` does).
pub fn mul(
    g: &mut Globals,
    r: &mut Run,
    a: Option<&Matrix>,
    b: Option<&Matrix>,
) -> Result<Option<Matrix>, Fatal> {
    Ok(larmorx_freesurfer_core::matrix::matrix_multiply(g, r.out, r.err, a, b, None)?.into_new())
}

/// `x = MatrixMultiply(a, b, x)`: the product into `x` (FreeSurfer copies an input that is `x`
/// first: the caller passes copies), `None` when `MatrixMultiply` returns `NULL`; a `None` `x`
/// gets a new matrix.
fn mul_into(
    g: &mut Globals,
    r: &mut Run,
    a: Option<&Matrix>,
    b: Option<&Matrix>,
    x: Option<Matrix>,
) -> Result<Option<Matrix>, Fatal> {
    match x {
        None => mul(g, r, a, b),
        Some(mut x) => {
            let ret = larmorx_freesurfer_core::matrix::matrix_multiply(
                g,
                r.out,
                r.err,
                a,
                b,
                Some(&mut x),
            )?;
            Ok(match ret {
                Ret::Null => None,
                _ => Some(x),
            })
        }
    }
}

/// `MatrixMultiply(x, b, x)` with the result ignored: `x` becomes `x b` (kept when the sizes do
/// not fit).
fn mul_left_in_place(
    g: &mut Globals,
    r: &mut Run,
    x: &mut Option<Matrix>,
    b: Option<&Matrix>,
) -> Result<(), Fatal> {
    let a = x.clone();
    match x {
        Some(m) => {
            larmorx_freesurfer_core::matrix::matrix_multiply(
                g,
                r.out,
                r.err,
                a.as_ref(),
                b,
                Some(m),
            )?;
        }
        None => {
            // MatrixMultiply(NULL, ...) ends the program
            larmorx_freesurfer_core::matrix::matrix_multiply(g, r.out, r.err, None, b, None)?;
        }
    }
    Ok(())
}

/// `MatrixInverse(m, NULL)`.
fn inverse(g: &mut Globals, r: &mut Run, m: Option<&Matrix>) -> Result<Option<Matrix>, Fatal> {
    Ok(matrix_inverse(g, r.out, r.err, m, None)?.into_new())
}

/// `MRIwrite(m, name)`'s status.
pub(crate) fn write_volume(
    g: &mut Globals,
    r: &mut Run,
    m: &mut Mri,
    name: &str,
) -> Result<i32, Fatal> {
    let env = r.env.clone();
    mri_write(
        g,
        r.out,
        r.err,
        &env,
        r.dir,
        m,
        name,
        None,
        MGZ_INTENT_UNKNOWN,
    )
}

/// `%lf` / `%f` of a `double`.
fn lf(x: f64) -> String {
    cfmt::f(x, 6)
}

/// `GetVmPeak()`'s line.
pub(crate) fn vmpc(g: &mut Globals, r: &mut Run) {
    let peak = get_vm_peak(g);
    let _ = writeln!(r.out, "#VMPC# mri_vol2vol VmPeak {peak}");
}

/// `main(argc, argv)` on the arguments after the program's name. `Ok` is status 0.
pub fn main(g: &mut Globals, r: &mut Run, args: &[String]) -> Result<(), Fatal> {
    g.vg_is_equal_threshold = 10e-4;
    let argv: Vec<String> = std::iter::once(NAME.to_owned())
        .chain(args.iter().cloned())
        .collect();
    let env = r.env.clone();
    let (nargs, left) = handle_version_option_whole(g, &env, r.out, &argv, NAME);
    if nargs != 0 && argv.len() as i32 - nargs == 1 {
        return Err(Fatal::Exit(0));
    }
    g.progname = NAME.to_owned();
    diag_init(g, &env, r.out);
    let Some(fsh) = r
        .env
        .get("FREESURFER_HOME")
        .map(|v| v.to_string_lossy().into_owned())
    else {
        let _ = writeln!(r.out, "ERROR: FREESURFER_HOME undefined.");
        return Err(Fatal::Exit(1));
    };
    if left.len() <= 1 {
        return Err(usage_exit(r));
    }
    let mut o = Options::new(fsh);
    parse_commandline(g, r, &mut o, &left[1..])?;
    if o.gdiagno > -1 {
        g.gdiag_no = o.gdiagno;
    }
    check_options(g, r, &mut o)?;
    if o.synth_seed < 0 {
        // the seed's unsigned long as an int (its low 32 bits)
        o.synth_seed = pdf_tod_seed(g) as u32 as i32;
    }
    g.drand48.srand48(i64::from(o.synth_seed));
    dump_options(g, r, &o);

    if o.do_crop {
        return crop(g, r, &mut o);
    }

    if o.fstal {
        let _ = writeln!(r.out);
        let _ = writeln!(r.out, "Compute R for talairach space");
        let subj = String::from_utf8_lossy(&o.subject_text()).into_owned();
        let env = r.env.clone();
        let mut io = GcaLtaIo { env: &env };
        let xtal = devolve_xfm(
            g,
            r.out,
            r.err,
            r.dir,
            &subj,
            None,
            Some(&o.talxfmfile),
            &mut io,
        )?
        .into_new();
        let inv_xtal = inverse(g, r, xtal.as_ref())?;
        let Some(xtal) = xtal else {
            return Err(Fatal::Exit(1));
        };
        let talres = o.fstalres;
        let rtal = if talres > 0 {
            load_rtal(g, r, &mut o, talres)?
        } else {
            identity(g, r)?
        };
        let inv_rtal = inverse(g, r, Some(&rtal))?;
        let _ = writeln!(r.out, "matrix from regfile ---------------------- ");
        let m = o.r.clone();
        print_matrix(g, r, m.as_ref());
        let _ = writeln!(r.out, "Xtal ---------------------- ");
        print_matrix(g, r, Some(&xtal));
        let _ = writeln!(r.out, "Rtal ---------------------- ");
        print_matrix(g, r, Some(&rtal));
        let a = o.r.clone();
        o.r = mul_into(g, r, a.as_ref(), inv_xtal.as_ref(), o.r.take())?;
        let a = o.r.clone();
        o.r = mul_into(g, r, a.as_ref(), inv_rtal.as_ref(), o.r.take())?;
        return resample(g, r, o, Some(rtal));
    }
    resample(g, r, o, None)
}

/// The `--crop` path (`main` 670-712).
fn crop(g: &mut Globals, r: &mut Run, o: &mut Options) -> Result<(), Fatal> {
    let _ = writeln!(r.out, "Crop {}", lf(o.crop_scale));
    let movname = o.movvolfile.clone().unwrap_or_default();
    let Some(mov) = read_volume(g, r, &movname)? else {
        return Err(Fatal::Exit(1));
    };
    let (err, bx) = mri_bounding_box(g, r.err, &mov, 0)?;
    if err != 0 {
        return Err(Fatal::Exit(1));
    }
    // FS-1181: FreeSurfer's end point is one past the box (a box of no voxel: FreeSurfer's points,
    // whose crop its allocator refuses)
    let past = [bx.x + bx.dx, bx.y + bx.dy, bx.z + bx.dz];
    let mut end = past;
    if bx.dx > 0 && bx.dy > 0 && bx.dz > 0 {
        g.notices.add(
            &FS_1181,
            "mri_vol2vol --crop: cut to the box of the voxels above 0 (FreeSurfer cuts one voxel more on each axis, and crashes when the box reaches the volume's last voxel)",
        );
        if !g.notices.reproduce() {
            end = [past[0] - 1, past[1] - 1, past[2] - 1];
        }
    }
    let Some(crop) = mri_crop(g, r.out, r.err, &mov, [bx.x, bx.y, bx.z], end)? else {
        // MRIcrop refused the end point; FreeSurfer then reads the NULL volume
        return Err(Fatal::Exit(139));
    };
    let s = o.crop_scale;
    let w = nint(g, f64::from(crop.width) * s);
    let h = nint(g, f64::from(crop.height) * s);
    let d = nint(g, f64::from(crop.depth) * s);
    if w <= 0 || h <= 0 || d <= 0 {
        g.notices.add(
            &FS_1182,
            format!(
                "mri_vol2vol --crop {}: a scale that leaves no voxel refused (FreeSurfer writes an empty volume or stops at the allocation)",
                lf(s)
            ),
        );
        if !g.notices.reproduce() {
            let _ = writeln!(
                r.out,
                "ERROR: --crop's scale {} leaves no voxel ({w} x {h} x {d})",
                lf(s)
            );
            return Err(Fatal::Exit(1));
        }
    }
    let mut cropnew = mri_alloc(g, r.out, r.err, w, h, d, mov.mtype)?;
    {
        let (c, n) = (&crop.vg, &mut cropnew.vg);
        n.x_r = c.x_r;
        n.x_a = c.x_a;
        n.x_s = c.x_s;
        n.y_r = c.y_r;
        n.y_a = c.y_a;
        n.y_s = c.y_s;
        n.z_r = c.z_r;
        n.z_a = c.z_a;
        n.z_s = c.z_s;
        n.c_r = c.c_r;
        n.c_a = c.c_a;
        n.c_s = c.c_s;
        n.xsize = (f64::from(c.xsize) / s) as f32;
        n.ysize = (f64::from(c.ysize) / s) as f32;
        n.zsize = (f64::from(c.zsize) / s) as f32;
    }
    let _ = writeln!(r.out, "vol2vol");
    let err = mri_vol2vol(
        g,
        r.out,
        r.err,
        &crop,
        &mut cropnew,
        None,
        SAMPLE_NEAREST,
        0.0,
    )?;
    if err != 0 {
        return Err(Fatal::Exit(1));
    }
    let _ = writeln!(r.out, "Saving");
    let outname = o.outvolfile.clone().unwrap_or_default();
    let err = write_volume(g, r, &mut cropnew, &outname)?;
    vmpc(g, r);
    let _ = writeln!(r.out, "mri_vol2vol done");
    if err != 0 {
        return Err(Fatal::Exit(err));
    }
    Ok(())
}

/// The rest of `main` (716-1310): the input and template, the registration, the resampling.
fn resample(
    g: &mut Globals,
    r: &mut Run,
    mut o: Options,
    rtal: Option<Matrix>,
) -> Result<(), Fatal> {
    let mut targ: Option<Mri>;
    let mut input: Mri;
    let mut template: Mri;
    let tempvolfile: Option<String>;
    if !o.invert {
        let movname = o.movvolfile.clone().unwrap_or_default();
        let Some(m) = read_volume(g, r, &movname)? else {
            return Err(Fatal::Exit(1));
        };
        targ = o.targ.take();
        if let Some(tv) = o.targvolfile.clone() {
            targ = read_header(g, r, &tv)?;
        } else if o.lta.is_some() && !o.fstal {
            let dst = o
                .lta
                .as_ref()
                .map(|l| l.xforms[0].dst.clone())
                .unwrap_or_default();
            match targ.as_mut() {
                Some(t) => {
                    mri_clone(g, r.out, r.err, &m, Some(t))?;
                }
                None => targ = mri_clone(g, r.out, r.err, &m, None)?.into_new(),
            }
            if let Some(t) = targ.as_mut() {
                mri_copy_vol_geom_to_mri(t, &dst);
            }
            o.usedltageom = true;
        } else {
            targ = Some(downsampled(g, r, &o, &m)?);
        }
        let Some(t) = targ.take() else {
            return Err(Fatal::Exit(1));
        };
        // in = mov; mri_template = targ (one volume each: the program never changes them apart)
        input = m;
        template = t;
        tempvolfile = o.targvolfile.clone();
    } else {
        let _ = writeln!(r.out);
        let _ = writeln!(r.out, "Invert!");
        let mut t = None;
        if let Some(tv) = o.targvolfile.clone() {
            t = read_volume(g, r, &tv)?;
        }
        let Some(t) = t else {
            return Err(Fatal::Exit(1));
        };
        let mut m = None;
        if let Some(mv) = o.movvolfile.clone() {
            m = read_header(g, r, &mv)?;
        } else if o.lta.is_some() && !o.fstal {
            let src = o
                .lta
                .as_ref()
                .map(|l| l.xforms[0].src.clone())
                .unwrap_or_default();
            m = mri_clone(g, r.out, r.err, &t, None)?.into_new();
            if let Some(mm) = m.as_mut() {
                mri_copy_vol_geom_to_mri(mm, &src);
            }
            o.usedltageom = true;
        }
        let Some(m) = m else {
            return Err(Fatal::Exit(1));
        };
        input = t;
        template = m;
        tempvolfile = o.movvolfile.clone();
    }
    // `mov` and `targ` as FreeSurfer names them: views of input and template
    macro_rules! movv {
        () => {
            if o.invert { &template } else { &input }
        };
    }
    macro_rules! targv {
        () => {
            if o.invert { &input } else { &template }
        };
    }
    if let Some(mut ctrl) = o.mri_soap_ctrl.take() {
        mri_binarize(
            g,
            r.out,
            r.err,
            Pair::Same(&mut ctrl),
            1.0,
            0.0,
            CONTROL_MARKED as f32,
        )?;
        let soap = mri_soap_bubble(
            g,
            r.out,
            r.err,
            Pair::New(movv!()),
            &ctrl,
            o.soap_bubble_iters,
            0.0,
        )?;
        if let Ret::New(mut s) = soap {
            let outname = o.outvolfile.clone().unwrap_or_default();
            write_volume(g, r, &mut s, &outname)?;
        }
        vmpc(g, r);
        return Err(Fatal::Exit(0));
    }
    if o.synth {
        let _ = writeln!(r.out);
        let _ = writeln!(r.out, "Replacing input data with synthetic white noise");
        let dims = [input.width, input.height, input.depth, input.nframes];
        mri_randn(g, r.out, r.err, dims, 0.0, 1.0, Some(&mut input))?;
    }
    if o.regheader {
        let _ = writeln!(r.out);
        let _ = writeln!(r.out, "Computing registration based on scanner-to-scanner");
        let (tv, mv) = (targv!().vg.clone(), movv!().vg.clone());
        o.r = Some(tkreg_mtx(g, r.out, r.err, &tv, &mv, o.xfm.as_ref())?);
    }
    if o.fslregfile.is_some() {
        let _ = writeln!(r.out);
        let _ = writeln!(r.out, "Computing registration based on fsl registration");
        let (tv, mv) = (targv!().vg.clone(), movv!().vg.clone());
        let env = r.env.clone();
        o.r = fsl2tkreg(g, r.out, r.err, &env, &tv, &mv, o.rfsl.as_ref())?;
        if o.rfsl2.is_some() {
            o.r2 = fsl2tkreg(g, r.out, r.err, &env, &tv, &mv, o.rfsl2.as_ref())?;
        }
    }
    if o.r.is_none() {
        error_exit(
            g,
            r.err,
            ERROR_BADPARM,
            "ERROR: no registration (R) is specified\n",
        )?;
    }
    if let Some(r2) = o.r2.clone() {
        let a = o.r.clone().unwrap_or_else(|| unreachable!("checked"));
        let mut x = o.r.take().unwrap_or_else(|| unreachable!("checked"));
        let ret = matrix_subtract(g, r.out, r.err, &a, &r2, Some(&mut x))?;
        o.r = match ret {
            Ret::Null => None,
            _ => Some(x),
        };
        // R->rptr[n][n] += 1.0 (FreeSurfer reads the NULL matrix when the sizes did not fit)
        let Some(m) = o.r.as_mut() else {
            return Err(Fatal::Exit(139));
        };
        for n in 1..4 {
            let v = m.relt(n, n);
            m.set_relt(n, n, (f64::from(v) + 1.0) as f32);
        }
    }
    let rad = |x: f64| x * 180.0 / std::f64::consts::PI;
    if let Some(mrot) = o.mrot.clone() {
        let _ = writeln!(r.out, "Applying rotation matrix (R=M*R)");
        let _ = writeln!(r.out, "Current Reg Matrix is:");
        let m = o.r.clone();
        print_matrix(g, r, m.as_ref());
        let a = o.angles;
        let _ = writeln!(
            r.out,
            "  Angles (deg): {} {} {}",
            lf(rad(a[0])),
            lf(rad(a[1])),
            lf(rad(a[2]))
        );
        let _ = writeln!(
            r.out,
            "  Angles (rad): {} {} {}",
            lf(a[0]),
            lf(a[1]),
            lf(a[2])
        );
        let _ = writeln!(r.out, "  Rotation matrix:");
        print_matrix(g, r, Some(&mrot));
        let b = o.r.clone();
        o.r = mul_into(g, r, Some(&mrot), b.as_ref(), o.r.take())?;
    }
    if let Some(mtrans) = o.mtrans.clone() {
        let _ = writeln!(r.out, "Applying translation matrix (R=M*R)");
        let _ = writeln!(r.out, "Current Reg Matrix is:");
        let m = o.r.clone();
        print_matrix(g, r, m.as_ref());
        let t = o.xyztrans;
        let _ = writeln!(
            r.out,
            "  Trans (mm): {} {} {}",
            lf(t[0]),
            lf(t[1]),
            lf(t[2])
        );
        let _ = writeln!(r.out, "  Translation matrix:");
        print_matrix(g, r, Some(&mtrans));
        let b = o.r.clone();
        o.r = mul_into(g, r, Some(&mtrans), b.as_ref(), o.r.take())?;
    }
    if let Some(mshear) = o.mshear.clone() {
        let _ = writeln!(r.out, "Applying shear matrix (R=M*R)");
        let _ = writeln!(r.out, "Current Reg Matrix is:");
        let m = o.r.clone();
        print_matrix(g, r, m.as_ref());
        let s = o.shear;
        let _ = writeln!(r.out, "  Shear: {} {} {}", lf(s[0]), lf(s[1]), lf(s[2]));
        let _ = writeln!(r.out, "  Shear matrix:");
        print_matrix(g, r, Some(&mshear));
        let b = o.r.clone();
        o.r = mul_into(g, r, Some(&mshear), b.as_ref(), o.r.take())?;
    }
    if let Some(f) = o.reg_file_final.clone() {
        let _ = writeln!(r.out, "Writing final tkRAS-to-tkRAS Matrix to {f}");
        let Some(m) = o.r.clone() else {
            return Err(Fatal::Exit(139));
        };
        let (xs, zs) = (input.xsize, input.zsize);
        regio_write_register(
            g,
            r.out,
            r.err,
            r.dir,
            &f,
            o.subject.as_deref(),
            xs,
            zs,
            1.0,
            &m,
            FLT2INT_ROUND,
        );
    }
    if o.invert {
        let _ = writeln!(r.out, "Inverting registration");
        let m = o.r.take();
        o.r = inverse(g, r, m.as_ref())?;
    }
    let _ = writeln!(r.out);
    let _ = writeln!(r.out, "Final tkRAS-to-tkRAS Matrix is:");
    let m = o.r.clone();
    print_matrix(g, r, m.as_ref());
    let _ = writeln!(r.out);

    if let Some(dispfile) = o.disp_file.clone() {
        let _ = writeln!(r.out, "Computing affine displacment");
        let Some(m) = o.r.as_ref() else {
            return Err(Fatal::Exit(139));
        };
        let mut disp = mri_affine_displacement(g, r.out, r.err, &input, m)?;
        // MRIwrite's status is not checked
        write_volume(g, r, &mut disp, &dispfile)?;
        if o.outvolfile.is_none() {
            let _ = writeln!(r.out, "No other output specified, so exiting now");
            vmpc(g, r);
            return Err(Fatal::Exit(0));
        }
    }

    let inv_r = inverse(g, r, o.r.as_ref())?;
    let tin = xfm_crs2xyz_tkreg(g, r.out, r.err, &input.vg)?;
    let inv_tin = inverse(g, r, Some(&tin))?;
    let ttemp = xfm_crs2xyz_tkreg(g, r.out, r.err, &template.vg)?;
    let inv_ttemp = inverse(g, r, Some(&ttemp))?;
    let sin = xfm_crs2xyz(g, r.out, r.err, &input.vg, 0, false)?;
    let _inv_sin = inverse(g, r, Some(&sin))?;
    let stemp = xfm_crs2xyz(g, r.out, r.err, &template.vg, 0, false)?;
    let _inv_stemp = inverse(g, r, Some(&stemp))?;

    if o.noresample {
        let _ = writeln!(r.out, "Not resampling, only changing vox2ras matrix");
        let mut vox2ras = mul(g, r, Some(&stemp), inv_ttemp.as_ref())?;
        mul_left_in_place(g, r, &mut vox2ras, inv_r.as_ref())?;
        mul_left_in_place(g, r, &mut vox2ras, Some(&tin))?;
        let Some(v2r) = vox2ras else {
            return Err(Fatal::Exit(139));
        };
        let env = r.env.clone();
        mri_set_voxel_to_ras_xform(g, r.out, r.err, &env, &mut input, &v2r)?;
        if o.slice_crop_flag {
            let _ = writeln!(
                r.out,
                "Cropping slices from {} to {}",
                o.slice_crop_start, o.slice_crop_stop
            );
            let Some(c) = mri_crop(
                g,
                r.out,
                r.err,
                &input,
                [0, 0, o.slice_crop_start],
                [input.width - 1, input.height - 1, o.slice_crop_stop],
            )?
            else {
                return Err(Fatal::Exit(1));
            };
            input = c;
        }
        if o.slice_reverse {
            let _ = writeln!(r.out, "Reversing slices, updating vox2ras");
            let env = r.env.clone();
            let Some(m) = mri_reverse_slices(g, r.out, r.err, &env, &input, None)?.into_new()
            else {
                return Err(Fatal::Exit(1));
            };
            input = m;
        }
        if o.slice_bias {
            let _ = writeln!(
                r.out,
                "Applying Half-Cosine Slice Bias, Alpha = {}",
                cfmt::g(o.slice_bias_alpha, 6)
            );
            let copy = mri_copy(g, r.out, r.err, &input, None)?.unwrap_new();
            mri_half_cos_bias(g, r.out, r.err, &copy, o.slice_bias_alpha, Some(&mut input))?;
        }
        let outname = o.outvolfile.clone().unwrap_or_default();
        let err = write_volume(g, r, &mut input, &outname)?;
        if err != 0 {
            let _ = writeln!(r.out, "ERROR: writing {outname}");
            return Err(Fatal::Exit(1));
        }
        let _ = writeln!(r.out, "To check registration, run:");
        let _ = writeln!(r.out);
        let _ = writeln!(
            r.out,
            "  tkregister2 --mov {outname} --targ {} --regheader --reg /tmp/reg ",
            or_null(tempvolfile.as_deref())
        );
        let _ = writeln!(r.out);
        let _ = writeln!(r.out, "mri_vol2vol done");
        return Ok(());
    }

    // vox2vox = invTin * R * Ttemp
    let mut vox2vox = mul(g, r, inv_tin.as_ref(), o.r.as_ref())?;
    mul_left_in_place(g, r, &mut vox2vox, Some(&ttemp))?;
    let _ = writeln!(r.out);
    let _ = writeln!(r.out, "Vox2Vox Matrix is:");
    print_matrix(g, r, vox2vox.as_ref());
    let _ = writeln!(r.out);

    template.mtype = o.precisioncode;
    if o.do_save_input_mr {
        template.tr = input.tr;
        template.ti = input.ti;
        template.flip_angle = input.flip_angle;
        template.te = input.te;
    }
    let mut out: Mri;
    if o.do_morph {
        let mov_vg = movv!().vg.clone();
        out = morph(g, r, &o, &input, &template.vg, &mov_vg)?;
    } else if o.do_kernel {
        out = mri_clone_by_space(g, r.out, r.err, &template, MRI_FLOAT, 8)?;
        let _ = writeln!(r.out, "Computing Trilinear Kernel");
        mri_vol2vol_tl_kernel(g, r.out, r.err, &input, &mut out, vox2vox.as_ref())?;
    } else if o.do_delta {
        let _ = writeln!(r.out, "Computing Delta");
        let Some(d) = mri_vol2vol_delta(g, r.out, r.err, &input, &template, o.r.as_ref())? else {
            return Err(Fatal::Exit(139));
        };
        out = d;
    } else if o.do_fill {
        let _ = writeln!(
            r.out,
            "Running MRIvol2VolFill(), DoConserve={}, US={}",
            i32::from(o.do_fill_conserve),
            o.fill_upsample
        );
        // !invert: TransformRegDat2LTA(targ, mov, R); invert: (mov, targ, R)
        let (a, b) = (template.vg.clone(), input.vg.clone());
        let Some(ltareg) = transform_reg_dat2_lta(g, r.out, r.err, &a, &b, o.r.as_ref())? else {
            return Err(Fatal::Exit(139));
        };
        let env = r.env.clone();
        let filled = mri_vol2vol_fill(
            g,
            r.out,
            r.err,
            &env,
            &mut input,
            None,
            &ltareg,
            o.fill_upsample,
            i32::from(o.do_fill_conserve),
            None,
        )?;
        let Some(f) = filled.into_new() else {
            return Err(Fatal::Exit(139));
        };
        out = f;
    } else {
        out = mri_clone_by_space(g, r.out, r.err, &template, -1, input.nframes)?;
        let _ = writeln!(r.out, "Resampling");
        if o.useold {
            mri_vol2vol(
                g,
                r.out,
                r.err,
                &input,
                &mut out,
                vox2vox.as_ref(),
                o.interpcode,
                o.sinchw as f32,
            )?;
        } else {
            let mut vsm = None;
            if let Some(v) = o.vsmvolfile.clone() {
                let _ = writeln!(r.out, "Reading {v}");
                vsm = read_volume(g, r, &v)?;
                if vsm.is_none() {
                    return Err(Fatal::Exit(1));
                }
            }
            if o.vsmlta.is_some() {
                let _ = writeln!(r.out, "Using vsmlta");
            }
            let env = r.env.clone();
            mri_vol2vol_vsm_env(
                g,
                r.out,
                r.err,
                &input,
                &mut out,
                vox2vox.as_ref(),
                o.interpcode,
                o.sinchw as f32,
                vsm.as_ref(),
                o.vsmlta.as_ref(),
                o.pedir,
                &env,
            )?;
        }
    }

    if let Some(costfile) = o.seg_reg_cost_file.clone() {
        seg_reg_cost_line(g, r, &o, &out, &costfile)?;
        if o.cost_only {
            return Err(Fatal::Exit(0));
        }
    }
    if o.slice_crop_flag {
        let _ = writeln!(
            r.out,
            "Cropping slices from {} to {}",
            o.slice_crop_start, o.slice_crop_stop
        );
        let Some(c) = mri_crop(
            g,
            r.out,
            r.err,
            &out,
            [0, 0, o.slice_crop_start],
            [out.width - 1, out.height - 1, o.slice_crop_stop],
        )?
        else {
            return Err(Fatal::Exit(1));
        };
        out = c;
    }
    if o.do_multiply {
        let _ = writeln!(r.out, "Multiplying by {}", lf(o.multiply_val));
        let copy = mri_copy(g, r.out, r.err, &out, None)?.unwrap_new();
        mri_const_op(g, r.out, r.err, &copy, o.multiply_val, true, Some(&mut out))?;
    }
    if let Some(c) = o.ctabfile.clone() {
        out.ct = ctab_read_ascii(g, r.out, r.err, r.dir, &c);
        if out.ct.is_none() {
            let _ = writeln!(r.out, "ERROR: reading {c}");
            return Err(Fatal::Exit(1));
        }
    } else {
        if movv!().ct.is_some() && !o.invert {
            out.ct = ctab_deep_copy(g, r.err, movv!().ct.as_ref());
        }
        if targv!().ct.is_some() && o.invert {
            out.ct = ctab_deep_copy(g, r.err, targv!().ct.as_ref());
        }
    }
    let outname = o.outvolfile.clone().unwrap_or_default();
    let err = write_volume(g, r, &mut out, &outname)?;
    if err != 0 {
        let _ = writeln!(r.out, "ERROR: writing {outname}");
        return Err(Fatal::Exit(1));
    }

    let subject_outreg: Vec<u8>;
    if let Some(rt) = rtal {
        o.r = Some(rt);
        subject_outreg = b"fsaverage".to_vec();
    } else {
        o.r = Some(identity(g, r)?);
        subject_outreg = o
            .subject
            .clone()
            .unwrap_or_else(|| b"subject-unknown".to_vec());
        let _ = writeln!(r.out, "Output registration matrix is identity");
    }
    if o.save_reg {
        let regfile0 = format!("{outname}.lta");
        if fixed_buffer(g, &regfile0, 1000, "--save-reg's name") {
            return Err(overflow_abort(r));
        }
        let _ = writeln!(r.out, "INFO: writing registration to {regfile0}");
        let mut lta = lta_alloc(g, r.out, r.err, 1, None)?;
        lta.xforms[0].sigma = 10000.0;
        lta.xforms[0].x0 = 0.0;
        lta.xforms[0].y0 = 0.0;
        lta.xforms[0].z0 = 0.0;
        lta.subject.strcpy_bytes(&subject_outreg);
        lta.xforms[0].m_l = o.r.clone();
        lta.ltype = REGISTER_DAT;
        lta.fscale = 1.0;
        lta_modify_src_dst_geom(g, r.out, r.err, &mut lta, Some(&input), Some(&template));
        let env = r.env.clone();
        lta_change_type(g, r.out, r.err, &env, &mut lta, LINEAR_VOX_TO_VOX)?;
        lta_write(g, r.out, r.err, &env, r.dir, &lta, &regfile0)?;
        let regfile0 = format!("{outname}.reg");
        let _ = writeln!(r.out, "INFO: writing registration matrix to {regfile0}");
        let m = o.r.clone().unwrap_or_else(|| unreachable!("set above"));
        regio_write_register(
            g,
            r.out,
            r.err,
            r.dir,
            &regfile0,
            Some(&subject_outreg),
            out.xsize,
            out.zsize,
            1.0,
            &m,
            FLT2INT_ROUND,
        );
        if !o.fstal {
            if !o.usedltageom {
                let _ = writeln!(r.out, "To check registration, run:");
                let _ = writeln!(r.out);
                let _ = writeln!(
                    r.out,
                    "  tkregister2 --mov {outname} --targ {} --reg {regfile0} ",
                    or_null(tempvolfile.as_deref())
                );
            }
        } else {
            let _ = writeln!(r.out, "To check registration, run:");
            let _ = writeln!(r.out);
            let _ = r.out.write_all(b"  tkregister2 --s ");
            let _ = r.out.write_all(&subject_outreg);
            let _ = writeln!(r.out, " --surf white --reg {regfile0} --mov {outname} ");
        }
    }
    let _ = writeln!(r.out);
    let _ = writeln!(r.out, "mri_vol2vol done");
    Ok(())
}

/// The `--downsample` target (`main` 744-761): the moving volume's header on a grid coarser by
/// the factors (`ceil` of the integer quotients), centred at 0 in the old way (`xstart = -xsize
/// width / 2`, ...). FS-1179: a factor of 0 or below for the second or third axis (the first is
/// never 0 here).
fn downsampled(g: &mut Globals, r: &mut Run, o: &Options, mov: &Mri) -> Result<Mri, Fatal> {
    let ds = o.down_sample;
    if ds.iter().any(|&f| f <= 0) {
        g.notices.add(
            &FS_1179,
            format!(
                "mri_vol2vol --downsample {} {} {}: a factor of 0 or below refused (FreeSurfer divides by 0 or makes a grid of negative size)",
                ds[0], ds[1], ds[2]
            ),
        );
        if !g.notices.reproduce() {
            let _ = writeln!(
                r.out,
                "ERROR: --downsample's factors must be 1 or more (got {} {} {})",
                ds[0], ds[1], ds[2]
            );
            return Err(Fatal::Exit(1));
        }
        if ds[1] == 0 || ds[2] == 0 {
            // the integer division by 0: SIGFPE
            return Err(Fatal::Exit(136));
        }
    }
    let mut t = mri_alloc_header(
        g,
        r.out,
        r.err,
        mov.width,
        mov.height,
        mov.depth,
        MRI_FLOAT,
        mov.nframes,
    )?;
    mri_copy_header(g, r.out, r.err, mov, Some(&mut t))?;
    mri_copy_pulse_parameters(mov, &mut t);
    // ceil(int / int): the integer quotient (truncated towards 0)
    t.vg.width = mov.width / ds[0];
    t.vg.height = mov.height / ds[1];
    t.vg.depth = mov.depth / ds[2];
    // float *= int: in float
    t.vg.xsize *= ds[0] as f32;
    t.vg.ysize *= ds[1] as f32;
    t.vg.zsize *= ds[2] as f32;
    let half = |size: f32, n: i32| -> (f32, f32) {
        let p = size * n as f32;
        (-p / 2.0, p / 2.0)
    };
    (t.xstart, t.xend) = half(t.xsize, t.width);
    (t.ystart, t.yend) = half(t.ysize, t.height);
    (t.zstart, t.zend) = half(t.zsize, t.depth);
    Ok(t)
}

/// The morphs (`main` 999-1072): `--morph` (`--m3z`): the morph `$SUBJECTS_DIR/<subject>/mri/
/// transforms/<m3z>` (`talairach.m3z` unless `--m3z`; with `--noDefM3zPath` the `--m3z` name as
/// it is) read (`GCAMread`, `Reading gcam`) and the input resampled onto its atlas
/// (`GCAMmorphToAtlas`, `Applying morph to input`); `--inv-morph`: the morph inverted on a
/// `float` volume of its image's grid (`GCAMinvert`), the registration (`TransformRegDat2LTA` of
/// the template, the moving volume and `R`, a voxel-to-voxel `TRANSFORM`) applied to its nodes
/// (`GCAMapplyTransform`, `Applying reg to gcam`: no effect on the inverse already made), and the
/// input resampled back onto the image (`GCAMmorphFromAtlas` with an atlas of the input's extent,
/// `gcaAllocMax(1, 1, 1, ...)`: every voxel through the inverse, clamped into the input; `Applying
/// inverse morph to input`). `--filter`: the mode filter of the result (`MRImodeFilter`, one pass,
/// `filtering labeled volume...`). FS-1191: a morph name of 1,000 bytes or more.
fn morph(
    g: &mut Globals,
    r: &mut Run,
    o: &Options,
    input: &Mri,
    template_vg: &VolGeom,
    mov_vg: &VolGeom,
) -> Result<Mri, Fatal> {
    let Some(ltareg) = transform_reg_dat2_lta(g, r.out, r.err, template_vg, mov_vg, o.r.as_ref())?
    else {
        return Err(Fatal::Exit(139));
    };
    let mut rtransform = Transform::new(LINEAR_VOX_TO_VOX, Xform::Lta(ltareg));
    let _ = writeln!(r.out, "Reading gcam");
    let gcamfile = if o.def_m3z_path {
        let sd = o.subjects_dir.clone().unwrap_or_default();
        let subj = String::from_utf8_lossy(&o.subject_text()).into_owned();
        format!("{sd}/{subj}/mri/transforms/{}", o.m3zfile)
    } else {
        o.m3zfile.clone()
    };
    if gcamfile.len() >= 1000 {
        g.notices.add(
            &FS_1191,
            format!(
                "mri_vol2vol: the morph's name of {} bytes used whole (FreeSurfer's buffer holds 999: the official build aborts)",
                gcamfile.len()
            ),
        );
        if g.notices.reproduce() {
            return Err(overflow_abort(r));
        }
    }
    let env = r.env.clone();
    let Some(mut gcam) = gcam_read(g, r.out, r.err, &env, r.dir, &gcamfile)? else {
        return Err(Fatal::Exit(1));
    };
    let out = if !o.invert_morph {
        let _ = writeln!(r.out, "Applying morph to input");
        gcam_morph_to_atlas(g, r.out, r.err, &env, input, &gcam, None, -1, o.interpcode)?.into_new()
    } else {
        let image = gcam.image.clone();
        let mut tmp = mri_alloc(
            g,
            r.out,
            r.err,
            image.width,
            image.height,
            image.depth,
            MRI_FLOAT,
        )?;
        use_vol_geom_to_mri(g, r.out, r.err, &image, &mut tmp.vg)?;
        gcam_invert(g, r.out, r.err, &env, r.dir, &mut gcam, Some(&tmp))?;
        drop(tmp);
        let _ = writeln!(r.out, "Applying reg to gcam");
        gcam_apply_transform(g, r.out, r.err, &env, &mut gcam, &mut rtransform)?;
        let _ = writeln!(r.out, "Applying inverse morph to input");
        let gca = gca_alloc_max(
            g,
            r.out,
            r.err,
            1,
            1.0,
            1.0,
            [input.vg.width, input.vg.height, input.vg.depth],
            0,
            0,
            None,
        )?;
        gcam_morph_from_atlas_gca(
            g,
            r.out,
            r.err,
            &env,
            r.dir,
            input,
            &mut gcam,
            Some(&gca),
            None,
            o.interpcode,
        )?
        .into_new()
    };
    let Some(mut out) = out else {
        return Err(Fatal::Exit(1));
    };
    if o.setfilter {
        let _ = writeln!(r.out, "filtering labeled volume...");
        out = mri_mode_filter(g, r.out, r.err, &out, None, 1)?;
    }
    Ok(out)
}

/// `--cost <file>` (`main` 1075-1092): the segmentation `$SUBJECTS_DIR/<subject>/mri/regseg`
/// (`IDnameFromStem`: `.nii`, `.nii.gz`, `.mgh`, `.mgz`, ...; read whole) and the resampled volume's
/// white matter and cortex statistics ([`seg_reg_cost`]), appended to the file as one line:
/// `--trans`'s three numbers (`%7.3lf `), `--rot`'s three in degrees (`%5.1lf `), the white
/// matter's count, mean and standard deviation (`%7d %10.4lf %8.4lf `), the cortex's, `t` and the
/// cost (`%8.4lf %8.4lf `). A segmentation that does not read ends the program (status 1).
/// FS-1193: no volume of that stem (FreeSurfer crashes); FS-1187: a cost refused (FreeSurfer
/// writes numbers it never computed); FS-1186: a file that does not open (FreeSurfer crashes).
fn seg_reg_cost_line(
    g: &mut Globals,
    r: &mut Run,
    o: &Options,
    out: &Mri,
    costfile: &str,
) -> Result<(), Fatal> {
    let sd = o.subjects_dir.clone().unwrap_or_default();
    let subj = String::from_utf8_lossy(&o.subject_text()).into_owned();
    let stem = format!("{sd}/{subj}/mri/regseg");
    if fixed_buffer(g, &stem, 2000, "the regseg segmentation's name") {
        return Err(overflow_abort(r));
    }
    let Some(fspec) = id_name_from_stem(g, r.dir, &stem) else {
        g.notices.add(
            &FS_1193,
            format!(
                "mri_vol2vol --cost: no volume {stem}.* (FreeSurfer reads a null name and crashes)"
            ),
        );
        if g.notices.reproduce() {
            return Err(Fatal::Exit(139));
        }
        let _ = writeln!(
            r.out,
            "ERROR: --cost: cannot find the segmentation {stem} (.nii, .nii.gz, .mgh, .mgz, ...)"
        );
        return Err(Fatal::Exit(1));
    };
    let Some(regseg) = read_volume(g, r, &fspec)? else {
        return Err(Fatal::Exit(1));
    };
    let mut costs: SegRegCosts = [0.0; 8];
    if !seg_reg_cost(g, r.out, r.err, &regseg, out, &mut costs)? {
        g.notices.add(
            &FS_1187,
            "mri_vol2vol --cost: the cost was refused (FreeSurfer writes eight numbers it never computed: memory contents)",
        );
        if !g.notices.reproduce() {
            return Err(Fatal::Exit(1));
        }
    }
    let mut fp = match CFile::open(r.dir, costfile, "a") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            g.notices.add(
                &FS_1186,
                format!("mri_vol2vol --cost: {costfile} does not open (FreeSurfer writes through a null file and crashes)"),
            );
            if g.notices.reproduce() {
                return Err(Fatal::Exit(139));
            }
            let _ = writeln!(r.out, "ERROR: could not open {costfile}");
            return Err(Fatal::Exit(1));
        }
    };
    let mut line = String::new();
    for t in o.xyztrans {
        line.push_str(&format!("{} ", fmt_f(t, 7, 3)));
    }
    for a in o.angles {
        let deg = div64(mul64(a, 180.0), std::f64::consts::PI);
        line.push_str(&format!("{} ", fmt_f(deg, 5, 1)));
    }
    for k in [0usize, 3] {
        let n = float_to_int(g, costs[k], "mri_vol2vol --cost");
        line.push_str(&format!(
            "{n:7} {} {} ",
            fmt_f(costs[k + 1], 10, 4),
            fmt_f(costs[k + 2], 8, 4)
        ));
    }
    line.push_str(&format!(
        "{} {} \n",
        fmt_f(costs[6], 8, 4),
        fmt_f(costs[7], 8, 4)
    ));
    let _ = fp.write_all(line.as_bytes());
    Ok(())
}
