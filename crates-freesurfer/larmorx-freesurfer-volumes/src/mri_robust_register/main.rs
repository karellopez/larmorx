// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_robust_register`'s `main` (`mri_robust_register.cpp` 592-1360) and `initRegistration`
//! (1435-1827), with `MyMRI::setTypeUCHAR` and `MyMRI::setMaxOutsideVal` (`MyMRI.cpp` 580-612):
//! the program's entry point [`run`] (registered as `mri_robust_register` in the crate's
//! `PROGRAMS`) and [`main`].
//!
//! The version line (`8.2.0`) and an empty line first; the version options (`handleVersionOption`:
//! the program ends with status 0 when nothing else is given); the command line
//! ([`super::options`]). The robust registration (`RegRobust`, the default cost): the source read
//! (`reading source '<mov>'...`; not read: `<progname>: could not open MRI source <mov>.`, more than
//! one frame `<progname>: only pass single frame MRI source <mov>.`, both `ERROR_NOFILE`), kept as
//! read; its mask read (`reading source mask '<mask>'...`) and applied (`MRImask`), or the Sobel
//! magnitude taken first (`--sobel`); with `--cost ROBENT` the image made its local entropy
//! instead of being masked (`Converting <mov|dst> to entropy image (radius <r> ) ... (can take 1-2
//! min)`, `MyMRI::entropyImage` with the mask: [`larmorx_freesurfer_robust::entropy`]; written
//! with `--entmov` / `--entdst`; the source's ` Entropy computation took <s> seconds.`, from the
//! clock); the same for the target. The initial transform (`--ixform`,
//! `TransformRead`: an LTA's first transform, its missing geometries taken from the images with a
//! warning, made RAS to RAS, its geometries replaced by the images' where they differ
//! (`LTAmodifySrcDstGeom`), made voxel to voxel); the initial intensity scale (`--iscalein`, the
//! file's first number as C++'s `>>` reads it; no file: `Load Iscale input: no such file ( <name> )`,
//! `ERROR_BADFILE`); `--uchar` (each image changed to `uchar` with its histogram's 0 to 99.9 %), the
//! white backgrounds (the maximum voxel as the outside value); then `setSourceAndTarget`, the
//! saturation search (`--satit`) and the multi-resolution registration (or the iterations on the
//! full images alone, `--nomulti`).
//!
//! The results: `Final Transform:` (`M`, VNL's long format), its determinant, for `--affine` or
//! `--isoscale` its decomposition into rotation, shear and scale, the intensity scale; the LTA
//! (RAS to RAS from the voxel transform, or voxel to voxel with `--vox2vox`; the images'
//! geometries) written by `LTAwriteEx`; the scale's file (`--iscaleout`); the source scaled by the
//! intensity factor; `--mapmov` (the source resampled onto the target by a cubic B-spline, the
//! source's type), `--mapmovhdr` (the source with its vox2ras moved onto the target; a write that
//! fails: `ERROR: writing <name>`, status 1), `--weights` (the weights in the target's space, as
//! `1 - w` unless `--oneminusw`), the half-way outputs (`--halfmov`, `--halfdst`, `--halfweights`,
//! `--halfmovlta`, `--halfdstlta`: the two half-way maps on standard error, the transforms to the
//! half-way space, the images resampled there); the time taken (`Registration took <m> minutes and
//! <s> seconds.`, from the clock) and the citation. Status 0.
//!
//! The other costs (`--cost` `LS`, `MI`, `NMI`, `ECC`, `NCC`, `SCR`, `TB`, `LNCC`, `SAD`, `SB`):
//! `RegPowell` ([`larmorx_freesurfer_robust::powell`]), the images allowed to change type
//! (`P.floattype`), the tolerance of `--powelltol`, no saturation search, no weights (`Warning:
//! no weights could be created! ...`); once Powell has printed its final error the program's
//! numbers print with 14 digits (`setprecision(14)` stays on `std::cout`); `SB`'s second citation.
//! Their half-way outputs take the geometry of the Powell registration's byte target of the last
//! level it registered (FS-1075); `--halfweights` then prints `Warning: no weights have been
//! computed! Maybe you ran with --leastsquares??`; with no level registered (the finest skipped,
//! `--highit 0`, and no coarser one) `ERROR: no half way space created (not implemented for this
//! cost function)!`.
//!
//! Not ported yet (each ends with its message and status 1): the test mode (`--test`) and the
//! debug output of `--debug` (the library's intermediate images and its key presses).
//!
//! Defects (`larmorx_core::defects`; `docs/findings/freesurfer-robust-register.md`):
//! - FS-1070: [`super::options`].
//! - **FS-1071** (crash): an `--lta` name of 1,024 bytes or more is copied into a buffer of 1,024
//!   (`strcpy`, checked by the official build): the program aborts (`*** buffer overflow detected
//!   ***: terminated`, status 134) after the results are printed. By default the name is used;
//!   `reproduce` aborts as FreeSurfer does.
//! - **FS-1072** (memory): an initial transform that is not linear (a morph, `--ixform` of an
//!   `.m3z`) is read as an LTA: FreeSurfer reads the morph's memory as a transform array. An error
//!   in both modes.
//! - **FS-1075** (memory: freed): `RegPowell::getHalfWayGeom` returns `tcf`, the byte copy of the
//!   target that `computeIterativeRegistrationFull` has freed (`MRIfree(&mriT)`) by the time
//!   `main` reads its header (`MRIvoxelXformToRasXform`, `getVolGeom`): the half-way LTAs'
//!   target geometry and the half-way images' grids are memory contents (measured, relay 48:
//!   garbage sizes and directions, runs that differ, an allocation that fails, status 1, or
//!   exhausts the memory box). Both modes keep that image's header (the voxels released where
//!   FreeSurfer frees the image) and use it, noted (rule A; the maps on standard error, the
//!   defined part, as FreeSurfer prints them).

use std::io::Write;

use larmorx_core::cfmt::ostream;
use larmorx_core::defects::{FS_1071, FS_1072, FS_1075};
use larmorx_core::vnl_fixed;
use larmorx_core::vnl_matrix::{VnlMatrix, matlab_print_long, matlab_print_long_diag};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADFILE, ERROR_NOFILE, Fatal};
use larmorx_freesurfer_core::matrix::{Matrix, matrix_copy, matrix_multiply};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Settings;
use larmorx_freesurfer_core::system;
use larmorx_freesurfer_core::timer::Timer;
use larmorx_freesurfer_core::utils::{nint, resolve};
use larmorx_freesurfer_core::version::{get_version, handle_version_option};
use larmorx_freesurfer_filter::sobel::mri_sobel;
use larmorx_freesurfer_gca::transform::{Xform, transform_read};
use larmorx_freesurfer_io::mriio::{mri_read_volume, mri_write};
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::lta::{LINEAR_RAS_TO_RAS, LINEAR_VOX_TO_VOX, Lta, lta_alloc};
use larmorx_freesurfer_mri::mri::{
    MGZ_INTENT_UNKNOWN, MRI_FLOAT, MRI_UCHAR, Mri, mri_alloc, mri_copy_header,
    mri_copy_pulse_parameters,
};
use larmorx_freesurfer_mri::vol_geom::extract_i_to_r;
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;
use larmorx_freesurfer_robust::entropy::{EntropyParams, entropy_image};
use larmorx_freesurfer_robust::mymatrix::{
    convert_matrix2vnl, convert_vnl2matrix, polar2_decomposition,
};
use larmorx_freesurfer_robust::mymri::mri_valscale;
use larmorx_freesurfer_robust::registration::{Io, RegRobust};
use larmorx_freesurfer_volume::arith::{Pair, mri_linear_scale};
use larmorx_freesurfer_volume::cma::istream_double;
use larmorx_freesurfer_volume::frames::mri_clone_different_type;
use larmorx_freesurfer_volume::ltaapply::{lta_modify_src_dst_geom, lta_transform_interp};
use larmorx_freesurfer_volume::ltaops::lta_change_type;
use larmorx_freesurfer_volume::ltawrite::lta_write_ex;
use larmorx_freesurfer_volume::mriset::mri_mask;
use larmorx_freesurfer_volume::orient::{mri_set_voxel_to_ras_xform, mri_voxel_xform_to_ras_xform};
use larmorx_freesurfer_volume::resize::mri_seq_change_type;
use larmorx_freesurfer_volume::vgeom::get_vol_geom;

use super::options::{CostName, Options, Run, parse_command_line};

/// The program's name as `argv[0]` gives it (fMRIPrep runs it by name).
pub const NAME: &str = "mri_robust_register";

/// `STRLEN`: `reg`, the buffer the LTA's name is copied into.
const STRLEN: usize = 1024;

/// `mri_robust_register`'s entry point: its original arguments (without the program's name), the
/// settings, the two streams; the exit status. The notices of the run follow on standard error.
pub fn run(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mut g = Globals::new(settings.mode);
    g.stderr.read_write = system::stderr_read_write();
    let status = {
        let mut terr = log::TrackedErr::new(&g, &mut *err);
        let mut r = Run {
            out: &mut *out,
            err: &mut terr,
            env: &settings.env,
            dir: settings.dir.as_deref(),
            threads: settings.threads,
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

/// `main(argc, argv)` on the arguments after the program's name. `Ok` is status 0.
pub fn main(g: &mut Globals, r: &mut Run, args: &[String]) -> Result<(), Fatal> {
    let _ = write!(r.out, "{}\n\n", get_version());
    let _ = r.out.flush();
    let argv: Vec<String> = std::iter::once(NAME.to_owned())
        .chain(args.iter().cloned())
        .collect();
    let (nargs, left) = handle_version_option(g, r.env, r.out, &argv, NAME);
    if nargs != 0 && left.len() == 1 {
        return Err(Fatal::Exit(0));
    }
    g.progname = NAME.to_owned();
    let mut p = Options::default();
    parse_command_line(g, r, &mut p, &left[1..])?;
    let mut start = Timer::new();
    start.reset();
    let mut reg = if p.robust() {
        RegRobust::new()
    } else {
        // the images may change type inside the registration (RegPowell makes them bytes)
        p.floattype = true;
        RegRobust::new_powell()
    };
    let (mov, dst) = init_registration(g, r, &mut reg, &mut p)?;
    let (mut mov, dst) = (mov, dst);
    {
        let mut io = Io {
            out: &mut *r.out,
            err: &mut *r.err,
            env: r.env,
        };
        if p.satit && p.robust() {
            reg.find_saturation(g, &mut io)?;
            reg.compute_multires_registration(g, &mut io, 0, p.iterate, p.epsit)?;
        } else if p.nomulti {
            reg.compute_iterative_registration(g, &mut io, p.iterate, p.epsit)?;
        } else {
            reg.compute_multires_registration(g, &mut io, 0, p.iterate, p.epsit)?;
        }
    }
    let _ = write!(r.out, "\nFinal Transform:\n");
    let fmv2v = reg.get_final_vox2vox(r.out);
    let fv = VnlMatrix::from_4x4(&fmv2v);
    let _ = writeln!(r.out, "{}", matlab_print_long(&fv, "M"));
    let md_first = convert_vnl2matrix(g, r.out, r.err, &fv)?;
    let md_second = reg.get_final_iscale();
    let flat: Vec<f64> = fmv2v.iter().flatten().copied().collect();
    let _ = write!(
        r.out,
        " Determinant : {}\n\n",
        reg.os(vnl_fixed::determinant(&flat, 4))
    );
    if p.affine || p.isoscale {
        let _ = write!(r.out, " Decompose into Rot * Shear * Scale : \n\n");
        let mut a = VnlMatrix::zeros(3, 3);
        for (i, row) in fmv2v.iter().take(3).enumerate() {
            for (j, &v) in row.iter().take(3).enumerate() {
                *a.at_mut(i, j) = v;
            }
        }
        let (rot, shear, scale) = polar2_decomposition(g, r.err, &a)?;
        let _ = writeln!(r.out, "{}", matlab_print_long(&rot, "Rot"));
        let _ = writeln!(r.out, "{}", matlab_print_long(&shear, "Shear"));
        let _ = writeln!(r.out, "{}", matlab_print_long_diag(&scale, "Scale"));
    }
    if reg.iscale {
        let _ = writeln!(r.out, "Intenstiy Scale Factor: {}", reg.os(md_second));
    }
    let _ = writeln!(r.out);

    // the transform
    let _ = writeln!(r.out, "writing output transformation to {} ...", p.lta);
    let overflow = p.lta.len() >= STRLEN;
    if overflow {
        g.notices.add(
            &FS_1071,
            format!(
                "an --lta name of {} bytes (FreeSurfer's buffer holds 1,023 and a NUL: the program aborts)",
                p.lta.len()
            ),
        );
        if g.notices.reproduce() {
            let _ = r.out.flush();
            let _ = r
                .err
                .write_all(b"*** buffer overflow detected ***: terminated\n");
            return Err(Fatal::Exit(134));
        }
    }
    let mut lta = lta_alloc(g, r.out, r.err, 1, Some(&mov))?;
    if !p.lta_vox2vox {
        let _ = writeln!(r.out, "converting VOX to RAS and saving RAS2RAS...");
        let mut m_l = lta.xforms[0].m_l.take();
        let got = mri_voxel_xform_to_ras_xform(
            g,
            r.out,
            r.err,
            &mov,
            Some(&dst),
            Some(&md_first),
            m_l.as_mut(),
        )?;
        lta.xforms[0].m_l = match got {
            Ret::New(m) => Some(m),
            Ret::Given => m_l,
            Ret::Null => None,
        };
        lta.ltype = LINEAR_RAS_TO_RAS;
    } else {
        let _ = writeln!(r.out, "saving VOX2VOX...");
        let mut m_l = lta.xforms[0].m_l.take();
        let got = matrix_copy(g, r.out, r.err, Some(&md_first), m_l.as_mut())?;
        lta.xforms[0].m_l = match got {
            Ret::New(m) => Some(m),
            Ret::Given => m_l,
            Ret::Null => None,
        };
        lta.ltype = LINEAR_VOX_TO_VOX;
    }
    get_vol_geom(&mov.vg, &mut lta.xforms[0].src);
    get_vol_geom(&dst.vg, &mut lta.xforms[0].dst);
    lta_write_ex(g, r.out, r.err, r.env, r.dir, &lta, &p.lta)?;

    if reg.iscale && md_second > 0.0 && !p.iscaleout.is_empty() {
        // ofstream f(name) << Md.second: nothing when the file cannot be created
        let path = resolve(r.dir, &p.iscaleout);
        if let Err(e) = std::fs::write(path, ostream(md_second)) {
            g.errno = log::errno_of(&e).unwrap_or(2);
        }
    }
    if reg.iscale && md_second > 0.0 && !p.entropy {
        let _ = writeln!(r.out, "Adjusting Intensity of MOV by {}", reg.os(md_second));
        mri_valscale(g, r.out, r.err, Pair::Same(&mut mov), md_second, 0.0)?;
    }

    if !p.warpout.is_empty() {
        let _ = writeln!(r.out);
        let _ = writeln!(r.out, "mapmov: resampling MOV to DST ...");
        let _ = writeln!(r.out, " copy ... ");
        let mut aligned = mri_clone_different_type(g, r.out, r.err, &dst, mov.mtype)?;
        let _ = writeln!(r.out, " transform ... ");
        let got = lta_transform_interp(
            g,
            r.out,
            r.err,
            r.env,
            &mov,
            Some(&mut aligned),
            &mut lta,
            p.finalsampletype,
        )?;
        let mut aligned = match got {
            Ret::New(m) => m,
            _ => aligned,
        };
        mri_copy_pulse_parameters(&mov, &mut aligned);
        mri_write(
            g,
            r.out,
            r.err,
            r.env,
            r.dir,
            &mut aligned,
            &p.warpout,
            None,
            MGZ_INTENT_UNKNOWN,
        )?;
        let _ = writeln!(r.out);
        let _ = writeln!(r.out, "To check aligned result, run:");
        let _ = writeln!(r.out, "  freeview -v {} {}", p.dst, p.warpout);
    }

    if !p.norlout.is_empty() {
        let _ = writeln!(r.out);
        let _ = writeln!(
            r.out,
            "mapmovhdr: Changing vox2ras MOV header (to map to DST) ..."
        );
        let ras2ras =
            mri_voxel_xform_to_ras_xform(g, r.out, r.err, &mov, Some(&dst), Some(&md_first), None)?
                .into_new();
        let vox2ras = extract_i_to_r(g, r.out, r.err, &mov.vg)?;
        let vox2ras =
            matrix_multiply(g, r.out, r.err, ras2ras.as_ref(), Some(&vox2ras), None)?.into_new();
        let mut aligned = mri_copy(g, r.out, r.err, &mov, None)?
            .into_new()
            .expect("MRIcopy's new volume");
        if let Some(v2r) = vox2ras.as_ref() {
            mri_set_voxel_to_ras_xform(g, r.out, r.err, r.env, &mut aligned, v2r)?;
        }
        let e = mri_write(
            g,
            r.out,
            r.err,
            r.env,
            r.dir,
            &mut aligned,
            &p.norlout,
            None,
            MGZ_INTENT_UNKNOWN,
        )?;
        if e != 0 {
            let _ = writeln!(r.out, "ERROR: writing {}", p.norlout);
            return Err(Fatal::Exit(1));
        }
        let _ = writeln!(r.out);
        let _ = writeln!(r.out, "To check aligned result, run:");
        let _ = writeln!(r.out, "  freeview -v {} {}", p.dst, p.norlout);
    }

    if !p.weightsout.is_empty() {
        if let Some(w) = reg.mri_weights.as_mut() {
            let _ = writeln!(r.out);
            let _ = writeln!(r.out, "Writing out Weights ...");
            if p.oneminusweights {
                let mut iw = mri_linear_scale(g, r.out, r.err, Pair::New(w), -1.0, 1.0, false)?
                    .into_new()
                    .expect("MRIlinearScale's new volume");
                mri_write(
                    g,
                    r.out,
                    r.err,
                    r.env,
                    r.dir,
                    &mut iw,
                    &p.weightsout,
                    None,
                    MGZ_INTENT_UNKNOWN,
                )?;
            } else {
                mri_write(
                    g,
                    r.out,
                    r.err,
                    r.env,
                    r.dir,
                    w,
                    &p.weightsout,
                    None,
                    MGZ_INTENT_UNKNOWN,
                )?;
            }
            let _ = writeln!(r.out, "... overlay the weights:");
            if !p.warpout.is_empty() {
                let _ = writeln!(
                    r.out,
                    "  freeview -v {} {} {}:colormap=heat",
                    p.dst, p.warpout, p.weightsout
                );
            } else if !p.norlout.is_empty() {
                let _ = writeln!(
                    r.out,
                    "  freeview -v {} {} {}:colormap=heat",
                    p.dst, p.norlout, p.weightsout
                );
            }
        } else {
            let _ = writeln!(
                r.out,
                "Warning: no weights could be created! Maybe you ran with --leastsquares??"
            );
        }
    }

    if !p.halfmov.is_empty()
        || !p.halfdst.is_empty()
        || !p.halfweights.is_empty()
        || !p.halfdstlta.is_empty()
        || !p.halfmovlta.is_empty()
    {
        let _ = writeln!(r.out);
        if !p.symmetry {
            let _ = writeln!(
                r.out,
                "ERROR: no half way space created (symmetry was switched off)!"
            );
        } else if reg.powell {
            // RegPowell's half-way geometry: its byte copy of the target of the last level
            // registered (none before a first level: the message below)
            match reg.powell_hw.take() {
                None => {
                    let _ = writeln!(
                        r.out,
                        "ERROR: no half way space created (not implemented for this cost function)!"
                    );
                }
                Some(hw) => {
                    let done = half_way(g, r, &mut reg, &p, &mov, &dst, &hw, true);
                    reg.powell_hw = Some(hw);
                    done?;
                }
            }
        } else if let Some(hw) = reg.mri_weights.take() {
            let done = half_way(g, r, &mut reg, &p, &mov, &dst, &hw, false);
            reg.mri_weights = Some(hw);
            done?;
        } else {
            let _ = writeln!(
                r.out,
                "ERROR: no half way space created (not implemented for this cost function)!"
            );
        }
    }

    let _ = writeln!(r.out);
    drop(lta);
    let msec = start.milliseconds();
    let seconds = nint(g, (msec as f32 / 1000.0) as f64);
    let (minutes, seconds) = (seconds / 60, seconds % 60);
    let _ = write!(
        r.out,
        "\nRegistration took {minutes} minutes and {seconds} seconds.\n"
    );
    let _ = write!(r.out, "\n Thank you for using RobustRegister! \n");
    let _ = write!(
        r.out,
        " If you find it useful and use it for a publication, please cite: \n\n"
    );
    let _ = writeln!(
        r.out,
        " Highly Accurate Inverse Consistent Registration: A Robust Approach"
    );
    let _ = writeln!(
        r.out,
        " M. Reuter, H.D. Rosas, B. Fischl.  NeuroImage 53(4):1181-1196, 2010."
    );
    let _ = writeln!(r.out, " http://dx.doi.org/10.1016/j.neuroimage.2010.07.020");
    let _ = write!(
        r.out,
        " http://reuter.mit.edu/papers/reuter-robreg10.pdf\n\n"
    );
    if p.cost == CostName::Sb {
        let _ = writeln!(r.out, " And for the segmentation based cost function:");
        let _ = writeln!(
            r.out,
            " Multimodal image registration through simultaneous segmentation,"
        );
        let _ = writeln!(
            r.out,
            " I. Aganj and B. Fischl. IEEE Signal Processing Letters 24(11):1661-1665, 2017."
        );
        let _ = write!(r.out, " https://doi.org/10.1109/LSP.2017.2754263\n\n");
    }
    Ok(())
}

/// The half-way outputs (`main` 942-1129): the two maps on standard error, the transforms into
/// the half-way space (`hw`'s geometry: the robust registration's weights, or the Powell
/// registration's byte target, `powell`), and the images and weights resampled there.
#[allow(clippy::too_many_arguments)]
fn half_way(
    g: &mut Globals,
    r: &mut Run,
    reg: &mut RegRobust,
    p: &Options,
    mov: &Mri,
    dst: &Mri,
    hw: &Mri,
    powell: bool,
) -> Result<(), Fatal> {
    let _ = writeln!(r.out, "Creating half way data ...");
    let (m1, m2) = reg.get_half_way_maps(g, r.err)?;
    let _ = writeln!(
        r.err,
        "{}",
        matlab_print_long(&VnlMatrix::from_4x4(&m1), "movhw")
    );
    let _ = writeln!(
        r.err,
        "{}",
        matlab_print_long(&VnlMatrix::from_4x4(&m2), "dsthw")
    );
    if powell {
        // FreeSurfer's half-way geometry is the byte target it has freed: memory contents
        g.notices.add(
            &FS_1075,
            "the half-way geometry of the Powell registration (FreeSurfer reads its freed copy of the target); the registration's target",
        );
    }
    // the weights (`R.getWeights()`): the robust registration's; none after the Powell one
    let weights = (!powell).then_some(hw);
    half_way_outputs(g, r, p, mov, dst, &m1, &m2, hw, weights)
}

/// [`half_way`]'s transforms and images, into the geometry `hw`; the weights resampled there
/// when there are any.
#[allow(clippy::too_many_arguments)]
fn half_way_outputs(
    g: &mut Globals,
    r: &mut Run,
    p: &Options,
    mov: &Mri,
    dst: &Mri,
    m1: &[[f64; 4]; 4],
    m2: &[[f64; 4]; 4],
    hw: &Mri,
    weights: Option<&Mri>,
) -> Result<(), Fatal> {
    let mut m2hw = lta_alloc(g, r.out, r.err, 1, Some(mov))?;
    let mut d2hw = lta_alloc(g, r.out, r.err, 1, Some(dst))?;
    let c1 = convert_vnl2matrix(g, r.out, r.err, &VnlMatrix::from_4x4(m1))?;
    let c2 = convert_vnl2matrix(g, r.out, r.err, &VnlMatrix::from_4x4(m2))?;
    for (lta, src, c) in [(&mut m2hw, mov, &c1), (&mut d2hw, dst, &c2)] {
        if !p.lta_vox2vox {
            let mut m_l = lta.xforms[0].m_l.take();
            let got = mri_voxel_xform_to_ras_xform(
                g,
                r.out,
                r.err,
                src,
                Some(hw),
                Some(c),
                m_l.as_mut(),
            )?;
            lta.xforms[0].m_l = match got {
                Ret::New(m) => Some(m),
                Ret::Given => m_l,
                Ret::Null => None,
            };
            lta.ltype = LINEAR_RAS_TO_RAS;
        } else {
            // MyMatrix::convertVNL2MATRIX(m, m_L): the numbers into the transform's matrix
            lta.xforms[0].m_l = Some(c.clone());
            lta.ltype = LINEAR_VOX_TO_VOX;
        }
        get_vol_geom(&src.vg, &mut lta.xforms[0].src);
        get_vol_geom(&hw.vg, &mut lta.xforms[0].dst);
    }
    if !p.halfmovlta.is_empty() {
        lta_write_ex(g, r.out, r.err, r.env, r.dir, &m2hw, &p.halfmovlta)?;
    }
    if !p.halfdstlta.is_empty() {
        lta_write_ex(g, r.out, r.err, r.env, r.dir, &d2hw, &p.halfdstlta)?;
    }
    let resampled = |g: &mut Globals,
                     r: &mut Run,
                     src: &Mri,
                     lta: &mut Lta,
                     name: &str,
                     pulse: bool,
                     oneminus: bool|
     -> Result<(), Fatal> {
        let mut m =
            lta_transform_interp(g, r.out, r.err, r.env, src, None, lta, p.finalsampletype)?
                .into_new()
                .expect("LTAtransformInterp's new volume");
        if pulse {
            mri_copy_pulse_parameters(src, &mut m);
        }
        if oneminus {
            mri_linear_scale(g, r.out, r.err, Pair::Same(&mut m), -1.0, 1.0, false)?;
        }
        mri_write(
            g,
            r.out,
            r.err,
            r.env,
            r.dir,
            &mut m,
            name,
            None,
            MGZ_INTENT_UNKNOWN,
        )?;
        Ok(())
    };
    if !p.halfmov.is_empty() {
        let _ = writeln!(r.out, " creating half-way movable ...");
        resampled(g, r, mov, &mut m2hw, &p.halfmov, true, false)?;
    }
    if !p.halfdst.is_empty() {
        let _ = writeln!(r.out, " creating half-way destination ...");
        resampled(g, r, dst, &mut d2hw, &p.halfdst, true, false)?;
    }
    if !p.halfweights.is_empty() {
        if let Some(w) = weights {
            let _ = writeln!(r.out, " saving half-way weights ...");
            resampled(g, r, w, &mut d2hw, &p.halfweights, false, p.oneminusweights)?;
        } else {
            let _ = writeln!(
                r.out,
                "Warning: no weights have been computed! Maybe you ran with --leastsquares??"
            );
        }
    }
    Ok(())
}

/// `MyMRI::setTypeUCHAR(mri)`: its message, then `MRISeqchangeType(mri, MRI_UCHAR, 0.0, 0.999,
/// FALSE)` (no result: `ERROR: MRISeqchangeType in MyMRI::setTypeUCHAR`, status 1).
fn set_type_uchar(g: &mut Globals, r: &mut Run, mri: &mut Mri) -> Result<Mri, Fatal> {
    let _ = writeln!(
        r.out,
        "       -- changing data type from {} to {} (noscale = 0)...",
        mri.mtype, MRI_UCHAR
    );
    match mri_seq_change_type(g, r.out, r.err, r.env, mri, MRI_UCHAR, 0.0, 0.999, 0)? {
        Some(m) => Ok(m),
        None => {
            let _ = writeln!(r.out, "ERROR: MRISeqchangeType in MyMRI::setTypeUCHAR");
            Err(Fatal::Exit(1))
        }
    }
}

/// `MyMRI::setMaxOutsideVal(mri)`: the largest voxel of frame 0 (from the first, as `float`) as
/// the outside value.
fn set_max_outside_val(g: &mut Globals, r: &mut Run, mri: &mut Mri) -> Result<(), Fatal> {
    let mut maxval = mri_get_vox_val(g, r.err, mri, 0, 0, 0, 0, 0)?;
    for d in 0..mri.vg.depth {
        for h in 0..mri.vg.height {
            for w in 0..mri.vg.width {
                let v = mri_get_vox_val(g, r.err, mri, w, h, d, 0, 0)?;
                if v > maxval {
                    maxval = v;
                }
            }
        }
    }
    mri.outside_val = f64::from(maxval);
    Ok(())
}

/// One image read for the registration (`initRegistration`'s source or target part): read,
/// single frame, kept as read; the mask read and applied, or the Sobel magnitude taken first.
/// Returns the kept copy and the image to register.
#[allow(clippy::too_many_arguments)]
fn read_image(
    g: &mut Globals,
    r: &mut Run,
    p: &Options,
    name: &str,
    mask: &str,
    what: &str,
    short: &str,
) -> Result<(Mri, Mri), Fatal> {
    let progname = g.progname.clone();
    let Some(mut m) = mri_read_volume(g, r.out, r.err, r.env, r.dir, name)? else {
        log::error_exit(
            g,
            r.err,
            ERROR_NOFILE,
            &format!("{progname}: could not open MRI {what} {name}.\n"),
        )?;
        unreachable!("ErrorExit returns only for code 0");
    };
    if m.nframes != 1 {
        log::error_exit(
            g,
            r.err,
            ERROR_NOFILE,
            &format!("{progname}: only pass single frame MRI {what} {name}.\n"),
        )?;
    }
    let kept = mri_copy(g, r.out, r.err, &m, None)?
        .into_new()
        .expect("MRIcopy's new volume");
    let mut mask_vol = None;
    if !mask.is_empty() {
        let _ = writeln!(r.out, "reading {what} mask '{mask}'...");
        mask_vol = mri_read_volume(g, r.out, r.err, r.env, r.dir, mask)?;
        if mask_vol.is_none() {
            log::error_exit(
                g,
                r.err,
                ERROR_NOFILE,
                &format!("{progname}: could not open mask volume {mask}.\n"),
            )?;
        }
    }
    if p.sobel {
        let _ = writeln!(r.out, "Converting {short} to sobel magnitute image ...");
        let temp = m;
        m = mri_alloc(
            g,
            r.out,
            r.err,
            temp.vg.width,
            temp.vg.height,
            temp.vg.depth,
            MRI_FLOAT,
        )?;
        mri_copy_header(g, r.out, r.err, &temp, Some(&mut m))?;
        m.mtype = MRI_FLOAT;
        mri_sobel(g, r.out, r.err, &temp, None, Some(&mut m))?;
    }
    if p.entropy {
        // the source's conversion is timed (the target's is not)
        let mut start = Timer::new();
        start.reset();
        let _ = writeln!(
            r.out,
            "Converting {short} to entropy image (radius {} ) ... (can take 1-2 min)",
            p.entroradius
        );
        let params = EntropyParams {
            radius: p.entroradius,
            ball: p.entball,
            correction: p.entcorrection,
            threads: r.threads,
        };
        let mut temp = m;
        m = entropy_image(g, r.out, r.err, r.env, &mut temp, params, mask_vol.as_ref())?;
        drop(temp);
        let name = if short == "mov" { &p.entmov } else { &p.entdst };
        if !name.is_empty() {
            mri_write(
                g,
                r.out,
                r.err,
                r.env,
                r.dir,
                &mut m,
                name,
                None,
                MGZ_INTENT_UNKNOWN,
            )?;
        }
        if short == "mov" {
            let msec = start.milliseconds();
            let seconds = nint(g, (msec as f32 / 1000.0) as f64);
            let _ = writeln!(r.out, " Entropy computation took {seconds} seconds.");
        }
    } else if let Some(mk) = mask_vol.as_ref() {
        mri_mask(g, r.out, r.err, r.env, Pair::Same(&mut m), mk, 0, 0.0)?;
    }
    Ok((kept, m))
}

/// `initRegistration(R, P)`: the settings, the images read, the initial transform and intensity,
/// `setSourceAndTarget`. Returns the source and target as read (`P.mri_mov`, `P.mri_dst`).
fn init_registration(
    g: &mut Globals,
    r: &mut Run,
    reg: &mut RegRobust,
    p: &mut Options,
) -> Result<(Mri, Mri), Fatal> {
    if !p.affine {
        reg.set_rigid();
    } else {
        reg.set_affine();
    }
    if p.transonly {
        reg.set_transonly();
    }
    if p.isoscale {
        reg.set_isoscale();
    }
    reg.iscale = p.iscale;
    if p.iscaleonly {
        reg.set_iscale_only();
    }
    reg.set_verbose(p.verbose);
    reg.set_debug(p.debug);
    reg.highit = p.highit;
    reg.inittransform = p.inittrans;
    reg.initorient = p.initorient;
    reg.initscaling = p.initscaling;
    reg.doubleprec = p.doubleprec;
    reg.symmetry = p.symmetry;
    reg.costfun = p.library_cost();
    if reg.class_name() == "RegRobust" {
        reg.sat = p.sat;
        reg.wlimit = p.wlimit;
    }
    if reg.class_name() == "RegPowell" {
        reg.xtol = p.powelltol;
        reg.ftol = p.powelltol;
    }
    // P.lta.rfind(".") > 0: the name without its extension (a dot at 0 keeps it)
    match p.lta.rfind('.') {
        Some(pos) if pos > 0 => reg.set_name(&p.lta[..pos]),
        _ => reg.set_name(&p.lta),
    }
    if p.subsamplesize > 0 {
        reg.subsamplesize = p.subsamplesize;
    }
    reg.minsize = p.minsize;
    reg.maxsize = p.maxsize;

    let _ = writeln!(r.out);
    let _ = writeln!(r.out, "reading source '{}'...", p.mov);
    let _ = r.out.flush();
    let (pmov, mut mov) = read_image(g, r, p, &p.mov.clone(), &p.maskmov.clone(), "source", "mov")?;
    let _ = writeln!(r.out, "reading target '{}'...", p.dst);
    let _ = r.out.flush();
    let (pdst, mut dst) = read_image(g, r, p, &p.dst.clone(), &p.maskdst.clone(), "target", "dst")?;

    if !p.transform.is_empty() {
        let _ = writeln!(r.out);
        let _ = writeln!(r.out, "reading initial transform '{}'...", p.transform);
        let progname = g.progname.clone();
        let trans = transform_read(g, r.out, r.err, r.env, r.dir, &p.transform)?;
        let Some(trans) = trans else {
            log::error_exit(
                g,
                r.err,
                ERROR_BADFILE,
                &format!("{progname}: could not read transform file {}", p.transform),
            )?;
            unreachable!("ErrorExit returns only for code 0");
        };
        let mut lta: Lta = match trans.xform {
            Xform::Lta(l) => l,
            Xform::Morph(_) => {
                g.notices.add(
                    &FS_1072,
                    "--ixform of a morph: FreeSurfer reads the morph as a transform array",
                );
                log::error_exit(
                    g,
                    r.err,
                    ERROR_BADFILE,
                    &format!(
                        "{progname}: {} is not a linear transform (FreeSurfer reads the morph as one here)\n",
                        p.transform
                    ),
                )?;
                unreachable!("ErrorExit returns only for code 0");
            }
        };
        if lta.xforms[0].src.valid == 0 {
            let _ = writeln!(
                r.out,
                " WARNING: no source geometry (RAS) in transform, assuming movable !!!"
            );
            get_vol_geom(&mov.vg, &mut lta.xforms[0].src);
        }
        if lta.xforms[0].dst.valid == 0 {
            let _ = writeln!(
                r.out,
                " WARNING: no target geometry (RAS) in transform, assuming destination !!!"
            );
            get_vol_geom(&dst.vg, &mut lta.xforms[0].dst);
        }
        lta_change_type(g, r.out, r.err, r.env, &mut lta, LINEAR_RAS_TO_RAS)?;
        lta_modify_src_dst_geom(g, r.out, r.err, &mut lta, Some(&pmov), Some(&pdst));
        lta_change_type(g, r.out, r.err, r.env, &mut lta, LINEAR_VOX_TO_VOX)?;
        let m: &Matrix = lta.xforms[0].m_l.as_ref().expect("the transform's matrix");
        reg.minit = Some(convert_matrix2vnl(m));
    }

    if !p.iscalein.is_empty() {
        let mut iscale = 1.0;
        let path = resolve(r.dir, &p.iscalein);
        // an ifstream opens a folder too (its reads then fail: the scale stays 1)
        match std::fs::read(&path) {
            Ok(text) => istream_double(&text, &mut iscale),
            Err(_) if path.is_dir() => {}
            Err(e) => {
                // the failed open's errno, which ErrorExit prints first
                g.errno = log::errno_of(&e).unwrap_or(2);
                log::error_exit(
                    g,
                    r.err,
                    ERROR_BADFILE,
                    &format!("Load Iscale input: no such file ( {} )", p.iscalein),
                )?;
            }
        }
        reg.iscaleinit = iscale;
    }

    let _ = writeln!(r.out);
    if p.uchartype {
        mov = set_type_uchar(g, r, &mut mov)?;
        dst = set_type_uchar(g, r, &mut dst)?;
    }
    if p.whitebgmov {
        set_max_outside_val(g, r, &mut mov)?;
    }
    if p.whitebgdst {
        set_max_outside_val(g, r, &mut dst)?;
    }
    let mut io = Io {
        out: &mut *r.out,
        err: &mut *r.err,
        env: r.env,
    };
    reg.set_source_and_target(g, &mut io, &mov, &dst, !p.floattype)?;
    Ok((pmov, pdst))
}
