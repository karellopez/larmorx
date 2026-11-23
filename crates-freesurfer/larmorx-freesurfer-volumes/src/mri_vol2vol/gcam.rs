// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_vol2vol --gcam` and `--gcam0`: the program's own `MRIvol2volGCAM` and `MRIvol2volGCAM0`
//! (`mri_vol2vol.cpp` 2413-2655 and 2660-2841; queue item 3.2-38 part 2, track A relay 54).
//!
//! Both resample a moving volume into a destination grid through a chain of transforms read
//! backwards: the destination voxel through the destination LTA (to the morph's atlas grid),
//! the morph (`GCAMsampleMorph`, to the image grid; none when no morph is given), the source LTA
//! (to the moving volume's grid) and a voxel shift map (the B0 distortion: the point moved along
//! the phase-encoding axis by the map's trilinear sample).
//!
//! Defects (`larmorx_core::defects`; the mechanisms in `docs/findings/freesurfer-mri-vol2vol.md`):
//! - **FS-1188**: `MRIvol2volGCAM` without a morph but with a source LTA refuses a destination
//!   LTA whose destination is the source LTA's source (a missing `else`): by default accepted.
//! - **FS-1189**: a phase-encoding direction other than 1, 2 or 3 writes outside the point.
//! - **FS-1190**: `MRIvol2volGCAM0` without a destination LTA crashes.
//! - **FS-482** (the voxel shift a refused map type never wrote) and **FS-930**, **FS-931** (a
//!   sample that failed keeps the previous voxel's value; the first, memory contents), as for
//!   the library's resamplings.

use std::io::Write;
use std::time::Instant;

use larmorx_core::defects::{FS_482, FS_1188, FS_1189, FS_1190};
use larmorx_freesurfer_core::affine::float_to_int;
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::{
    MATRIX_REAL, Matrix, matrix_alloc, matrix_copy, matrix_multiply, matrix_multiply_d,
};
use larmorx_freesurfer_core::matrix_ops::{matrix_identity, matrix_inverse};
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::sse::add;
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_gca::gcam::Gcam;
use larmorx_freesurfer_gca::morph::{Chain, UNWRITTEN};
use larmorx_freesurfer_gca::transform::gcam_sample_morph;
use larmorx_freesurfer_mri::lta::{LINEAR_VOX_TO_VOX, Lta, lta_copy};
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_INT, MRI_LONG, MRI_SHORT, MRI_UCHAR, MRI_USHRT, Mri, SAMPLE_CUBIC_BSPLINE,
    SAMPLE_TRILINEAR, mri_alloc_from_vol_geom, mri_copy_pulse_parameters, mri_interp_string,
};
use larmorx_freesurfer_mri::vol_geom::vgprint;
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::bspline::{MriBspline, mri_sample_bspline, mri_to_bspline};
use larmorx_freesurfer_volume::combine::mri_dim_mismatch;
use larmorx_freesurfer_volume::coords::index_not_in_volume;
use larmorx_freesurfer_volume::ltaops::{
    InvertInto, lta_change_type, lta_invert, lta_mri_is_source, lta_mri_is_target, lta_print,
};
use larmorx_freesurfer_volume::sample::sample_volume_frame_type;
use larmorx_freesurfer_volume::seq::sample_seq_volume;
use larmorx_freesurfer_volume::vgeom::vg_is_equal;
use larmorx_freesurfer_volume::vol2vol::transform_reg_dat2_lta;

/// The streams and environment the two functions use.
pub struct Io<'a> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
    pub env: &'a Environment,
}

fn inverse(g: &mut Globals, io: &mut Io, m: Option<&Matrix>) -> Result<Option<Matrix>, Fatal> {
    Ok(matrix_inverse(g, io.out, io.err, m, None)?.into_new())
}

fn copy(g: &mut Globals, io: &mut Io, m: Option<&Matrix>) -> Result<Option<Matrix>, Fatal> {
    Ok(matrix_copy(g, io.out, io.err, m, None)?.into_new())
}

fn first_matrix(l: &Lta) -> Option<&Matrix> {
    l.xforms.first().and_then(|x| x.m_l.as_ref())
}

/// `src->vgprint()`.
fn print_geom(io: &mut Io, m: &Mri) {
    vgprint(io.out, &m.vg, false);
}

/// `LTAprint(stdout, lta)`.
fn print_lta(g: &mut Globals, io: &mut Io, l: &Lta) -> Result<(), Fatal> {
    lta_print(g, io.err, io.out, l)?;
    Ok(())
}

/// The voxel types `MRIsampleSeqVolume` reads.
fn sampled_type(ty: i32) -> bool {
    matches!(
        ty,
        MRI_UCHAR | MRI_FLOAT | MRI_SHORT | MRI_USHRT | MRI_INT | MRI_LONG
    )
}

/// The voxel shift (`float dvsm`, kept from voxel to voxel: `None` never written).
struct Shift {
    dvsm: Option<f32>,
    what: &'static str,
}

impl Shift {
    /// `MRIsampleSeqVolume(vsm, c, r, s, &dvsm, 0, 0)`: the shift after the sample.
    fn sample(
        &mut self,
        g: &mut Globals,
        io: &mut Io,
        vsm: &Mri,
        p: (f32, f32, f32),
    ) -> Result<f32, Fatal> {
        let (x, y, z) = (f64::from(p.0), f64::from(p.1), f64::from(p.2));
        let written = sampled_type(vsm.mtype) || index_not_in_volume(vsm, x, y, z) == 1;
        let mut one = [self.dvsm.unwrap_or(0.0)];
        sample_seq_volume(g, io.err, vsm, x, y, z, &mut one, 0, 0)?;
        if self.dvsm.is_none() && !written {
            g.notices.add(
                &FS_482,
                format!(
                    "{}: uses a voxel shift its sampler never wrote (memory contents in FreeSurfer)",
                    self.what
                ),
            );
        }
        if written || self.dvsm.is_some() {
            self.dvsm = Some(one[0]);
        }
        Ok(one[0])
    }
}

/// The four voxel shift map voxels around the point (`floor` of the column and row, `nint` of
/// the slice) inside the map and none of them 0 (`|v| < FLT_MIN`): the map's sample there,
/// `None` to skip the voxel.
fn shift_at(
    g: &mut Globals,
    io: &mut Io,
    shift: &mut Shift,
    src: &Mri,
    vsm: &Mri,
    p: (f32, f32, f32),
) -> Result<Option<f32>, Fatal> {
    let what = shift.what;
    let cvsm = float_to_int(g, f64::from(p.0).floor(), what);
    let rvsm = float_to_int(g, f64::from(p.1).floor(), what);
    let iss = nint(g, f64::from(p.2));
    let (w, h, d) = (src.vg.width, src.vg.height, src.vg.depth);
    if cvsm < 0 || cvsm.wrapping_add(1) >= w {
        return Ok(None);
    }
    if rvsm < 0 || rvsm.wrapping_add(1) >= h {
        return Ok(None);
    }
    if iss < 0 || iss.wrapping_add(1) >= d {
        return Ok(None);
    }
    let tiny = f64::from(f32::MIN_POSITIVE);
    for (c, r) in [
        (cvsm, rvsm),
        (cvsm + 1, rvsm),
        (cvsm, rvsm + 1),
        (cvsm + 1, rvsm + 1),
    ] {
        let v = mri_get_vox_val(g, io.err, vsm, c, r, iss, 0, 0)?;
        if f64::from(v).abs() < tiny {
            return Ok(None);
        }
    }
    let dvsm = shift.sample(g, io, vsm, p)?;
    if dvsm == 0.0 {
        return Ok(None);
    }
    Ok(Some(dvsm))
}

/// The samples of one destination voxel at the source point `p` (every frame): trilinear for
/// all frames at once (`MRIsampleSeqVolume`), the cubic B-spline's coefficients, or
/// `MRIsampleVolumeFrameType`; written into `dst`.
#[allow(clippy::too_many_arguments)]
fn sample_into(
    g: &mut Globals,
    io: &mut Io,
    src: &Mri,
    bspline: Option<&MriBspline>,
    chain: &mut Chain,
    valvect: &mut [f32],
    dst: &mut Mri,
    v: (i32, i32, i32),
    p: (f32, f32, f32),
    sample_type: i32,
) -> Result<(), Fatal> {
    let (x, y, z) = (f64::from(p.0), f64::from(p.1), f64::from(p.2));
    if sample_type != SAMPLE_TRILINEAR {
        for f in 0..src.nframes {
            let mut val = f64::from_bits(UNWRITTEN);
            if sample_type == SAMPLE_CUBIC_BSPLINE {
                let b = bspline.expect("the B-spline coefficients");
                mri_sample_bspline(g, io.out, io.err, b, x, y, z, f, &mut val)?;
            } else {
                sample_volume_frame_type(g, io.err, src, x, y, z, f, sample_type, &mut val);
            }
            let val = chain.take(g, val);
            mri_set_vox_val(g, io.err, dst, v.0, v.1, v.2, f, val as f32, 0)?;
        }
    } else {
        sample_seq_volume(g, io.err, src, x, y, z, valvect, 0, src.nframes - 1)?;
        for f in 0..src.nframes {
            mri_set_vox_val(g, io.err, dst, v.0, v.1, v.2, f, valvect[f as usize], 0)?;
        }
    }
    Ok(())
}

/// `printf("MRIvol2volGCAM(): interpolation type is %d %s\n", ...)`.
fn interp_line(io: &mut Io, sample_type: i32) {
    let _ = writeln!(
        io.out,
        "MRIvol2volGCAM(): interpolation type is {sample_type} {}",
        mri_interp_string(sample_type).unwrap_or("(null)")
    );
}

/// The loop both functions share: each destination voxel (columns slowest) through `vdst`
/// (`MatrixMultiplyD`), the morph (`GCAMsampleMorph`: a point outside it is skipped), `vsrc`
/// (`MatrixMultiply`), the voxel shift (`dir`), the source's extent, the sample.
#[allow(clippy::too_many_arguments)]
fn resample(
    g: &mut Globals,
    io: &mut Io,
    src: &Mri,
    gcam: Option<&Gcam>,
    vsm: Option<&Mri>,
    vdst: &Matrix,
    vsrc: &Matrix,
    dst: &mut Mri,
    sample_type: i32,
    dir: Dir,
    what: &'static str,
) -> Result<(), Fatal> {
    let bspline = if sample_type == SAMPLE_CUBIC_BSPLINE {
        Some(mri_to_bspline(g, io.out, io.err, src, None, 3)?)
    } else {
        None
    };
    let mut valvect = vec![0.0f32; src.nframes.max(0) as usize];
    let mut crs_dst = matrix_alloc(g, io.out, io.err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 vector");
    crs_dst.set_relt(4, 1, 1.0);
    let mut crs_anat = matrix_alloc(g, io.out, io.err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 vector");
    crs_anat.set_relt(4, 1, 1.0);
    let mut crs_gcam = Matrix::real(4, 1);
    let mut crs_src = Matrix::real(4, 1);
    let mut chain = Chain::new(what);
    let mut shift = Shift { dvsm: None, what };
    let (w, h, d) = (src.vg.width, src.vg.height, src.vg.depth);
    for c in 0..dst.vg.width.max(0) {
        for r in 0..dst.vg.height.max(0) {
            for s in 0..dst.vg.depth.max(0) {
                crs_dst.set_relt(1, 1, c as f32);
                crs_dst.set_relt(2, 1, r as f32);
                crs_dst.set_relt(3, 1, s as f32);
                matrix_multiply_d(
                    g,
                    io.out,
                    io.err,
                    Some(vdst),
                    Some(&crs_dst),
                    Some(&mut crs_gcam),
                )?;
                if let Some(gc) = gcam {
                    let (out_of_gcam, p) = gcam_sample_morph(
                        g,
                        io.err,
                        gc,
                        crs_gcam.relt(1, 1),
                        crs_gcam.relt(2, 1),
                        crs_gcam.relt(3, 1),
                    )?;
                    if out_of_gcam != 0 {
                        continue;
                    }
                    crs_anat.set_relt(1, 1, p[0]);
                    crs_anat.set_relt(2, 1, p[1]);
                    crs_anat.set_relt(3, 1, p[2]);
                } else {
                    matrix_copy(g, io.out, io.err, Some(&crs_gcam), Some(&mut crs_anat))?;
                }
                matrix_multiply(
                    g,
                    io.out,
                    io.err,
                    Some(vsrc),
                    Some(&crs_anat),
                    Some(&mut crs_src),
                )?;
                if let Some(vsm) = vsm {
                    let p = (crs_src.relt(1, 1), crs_src.relt(2, 1), crs_src.relt(3, 1));
                    let Some(dvsm) = shift_at(g, io, &mut shift, src, vsm, p)? else {
                        continue;
                    };
                    match dir {
                        Dir::Axis(axis, sign) => {
                            let e = crs_src.relt_mut(axis, 1);
                            *e = add(*e, if sign < 0.0 { -dvsm } else { dvsm });
                        }
                        // the point's homogeneous coordinate (made again for the next voxel)
                        Dir::Homogeneous => {}
                        Dir::Crash => return Err(Fatal::Exit(139)),
                    }
                }
                let p = (crs_src.relt(1, 1), crs_src.relt(2, 1), crs_src.relt(3, 1));
                if p.0 < 0.0 || p.0 >= w as f32 {
                    continue;
                }
                if p.1 < 0.0 || p.1 >= h as f32 {
                    continue;
                }
                if p.2 < 0.0 || p.2 >= d as f32 {
                    continue;
                }
                sample_into(
                    g,
                    io,
                    src,
                    bspline.as_ref(),
                    &mut chain,
                    &mut valvect,
                    dst,
                    (c, r, s),
                    p,
                    sample_type,
                )?;
            }
        }
    }
    Ok(())
}

/// Where the voxel shift goes: along an axis (1 column, 2 row, 3 slice) with a sign, into the
/// point's homogeneous coordinate (no effect), or outside the point (FreeSurfer crashes).
#[derive(Clone, Copy)]
enum Dir {
    Axis(i32, f32),
    Homogeneous,
    Crash,
}

/// The shift's direction for `pedir` (FS-1189): 1, 2, 3 along that axis; by default -1, -2, -3
/// the negative shift along that axis and others an error (`ERROR: MRIvol2volGCAM: pedir=<p>,
/// must be +/-1, +/-2, +/-3`, status 1); `reproduce` FreeSurfer's: a negative direction squares
/// the shift and writes it outside the point, 0 through a null row, 5 and more past the point (a
/// crash at the first voxel shifted), 4 into the point's homogeneous coordinate (no effect).
fn shift_dir(g: &mut Globals, io: &mut Io, pedir: i32) -> Result<Dir, Fatal> {
    if (1..=3).contains(&pedir) {
        return Ok(Dir::Axis(pedir, 1.0));
    }
    g.notices.add(
        &FS_1189,
        format!(
            "MRIvol2volGCAM: pedir={pedir}, not 1, 2 or 3 (FreeSurfer writes outside the point's coordinates)"
        ),
    );
    if g.notices.reproduce() {
        return Ok(if pedir == 4 {
            Dir::Homogeneous
        } else {
            Dir::Crash
        });
    }
    if (-3..=-1).contains(&pedir) {
        return Ok(Dir::Axis(-pedir, -1.0));
    }
    let _ = writeln!(
        io.out,
        "ERROR: MRIvol2volGCAM: pedir={pedir}, must be +/-1, +/-2, +/-3"
    );
    Err(Fatal::Exit(1))
}

/// `MRIvol2volGCAM(src, srclta, gcam, dstlta, vsm, sample_type, NULL, pedir)`: the new
/// destination volume (`float`, the source's frames and pulse parameters, the geometry of the
/// destination LTA's end that is not the atlas), or `None` after its error. The LTAs and the morph
/// are not changed (copies are inverted and converted to voxel-to-voxel).
#[allow(clippy::too_many_arguments)]
pub fn mri_vol2vol_gcam(
    g: &mut Globals,
    io: &mut Io,
    src: &Mri,
    srclta: Option<&Lta>,
    gcam: Option<&Gcam>,
    dstlta: Option<&Lta>,
    vsm: Option<&Mri>,
    sample_type: i32,
    pedir: i32,
) -> Result<Option<Mri>, Fatal> {
    let timer = Instant::now();
    let _ = writeln!(
        io.out,
        "MRIvol2volGCAM(): ---------+++++++++++++++++++----------------------"
    );
    if vsm.is_none() {
        let _ = writeln!(io.out, "MRIvol2volGCAM(): VSM not used");
    }
    if gcam.is_none() {
        let _ = writeln!(io.out, "MRIvol2volGCAM(): GCAM not used");
    }
    if srclta.is_none() {
        let _ = writeln!(io.out, "MRIvol2volGCAM(): Source LTA not used");
    }
    interp_line(io, sample_type);
    if let Some(v) = vsm
        && mri_dim_mismatch(src, v, false) != 0
    {
        let _ = writeln!(io.out, "ERROR: MRIvol2volGCAM(): src-vsm dim mismatch");
        return Ok(None);
    }
    let made;
    let dstlta: &Lta = match dstlta {
        Some(l) => l,
        None => {
            let Some(gc) = gcam else {
                let _ = writeln!(
                    io.out,
                    "ERROR: MRIvol2volGCAM(): both dstlta and gcam are NULL"
                );
                return Ok(None);
            };
            let _ = writeln!(
                io.out,
                "Dst LTA is null, so just using gcam atas as output target"
            );
            let Some(l) = transform_reg_dat2_lta(g, io.out, io.err, &gc.atlas, &gc.atlas, None)?
            else {
                return Err(Fatal::Exit(139));
            };
            made = l;
            &made
        }
    };
    // the source LTA, pointing from the image (or the destination) to the moving volume
    let mut srccopy: Option<Lta> = None;
    if let Some(sl) = srclta {
        let mut c = lta_copy(g, io.out, io.err, sl, None)?
            .into_new()
            .expect("LTAcopy makes a new array");
        if lta_mri_is_source(g, io.err, &c, src)? != 0 {
            let _ = writeln!(io.out, "MRIvol2volGCAM(): Inverting Source LTA");
            lta_invert(g, io.out, io.err, InvertInto::Given(sl, &mut c))?;
        } else if lta_mri_is_target(g, io.err, &c, src)? == 0 {
            let _ = writeln!(
                io.out,
                "ERROR: MRIvol2volGCAM(): neither src nor dst of Source LTA matches the mov vol geom"
            );
            let _ = writeln!(io.out, "mov vol geom ======================");
            print_geom(io, src);
            let _ = writeln!(io.out, "Source LTA ======================");
            print_lta(g, io, &c)?;
            let _ = writeln!(
                io.out,
                "ERROR: MRIvol2volGCAM(): neither src nor dst of Source LTA matches the mov vol geom"
            );
            return Ok(None);
        }
        if let Some(gc) = gcam
            && vg_is_equal(g, &c.xforms[0].src, &gc.image) == 0
        {
            let _ = writeln!(
                io.out,
                "ERROR: MRIvol2volGCAM(): gcam image vol geom does not match Source LTA src vol geom"
            );
            let _ = writeln!(io.out, "gcam image vol geom ======================");
            vgprint(io.out, &gc.image, false);
            let _ = writeln!(io.out, "Source LTA ======================");
            print_lta(g, io, &c)?;
            let _ = writeln!(
                io.out,
                "ERROR: MRIvol2volGCAM(): gcam image vol geom does not match Source LTA src vol geom"
            );
            return Ok(None);
        }
        if c.ltype != LINEAR_VOX_TO_VOX {
            let _ = writeln!(
                io.out,
                "MRIvol2volGCAM(): Chaning Source LTA type to vox2vox"
            );
            let env = io.env;
            lta_change_type(g, io.out, io.err, env, &mut c, LINEAR_VOX_TO_VOX)?;
        }
        srccopy = Some(c);
    }
    let vsrc = match &srccopy {
        Some(c) => copy(g, io, first_matrix(c))?,
        None => Some(matrix_identity(g, io.out, io.err, 4, None)?.unwrap_new()),
    };
    // the destination LTA, pointing from the destination to the atlas (or to the image)
    let invert_dst;
    if let Some(gc) = gcam {
        if vg_is_equal(g, &dstlta.xforms[0].dst, &gc.atlas) != 0 {
            invert_dst = false;
        } else if vg_is_equal(g, &dstlta.xforms[0].src, &gc.atlas) != 0 {
            invert_dst = true;
        } else {
            let _ = writeln!(
                io.out,
                "ERROR: MRIvol2volGCAM(): neither src nor dst of Dest LTA matches the atlas vol geom in the GCAM"
            );
            let _ = writeln!(io.out, "gcam atlas vol geom ======================");
            vgprint(io.out, &gc.atlas, false);
            let _ = writeln!(io.out, "Dest LTA ======================");
            print_lta(g, io, dstlta)?;
            let _ = writeln!(
                io.out,
                "ERROR: MRIvol2volGCAM(): neither src nor dst of Dest LTA matches the atlas vol geom in the GCAM"
            );
            return Ok(None);
        }
    } else if let Some(c) = &srccopy {
        let dst_match = vg_is_equal(g, &dstlta.xforms[0].dst, &c.xforms[0].src) != 0;
        let src_match = vg_is_equal(g, &dstlta.xforms[0].src, &c.xforms[0].src) != 0;
        if src_match {
            invert_dst = true;
        } else if dst_match {
            // FreeSurfer's second test has no `else` before it: its `else` refuses this case
            g.notices.add(
                &FS_1188,
                "MRIvol2volGCAM: the destination LTA's destination is the source LTA's source (FreeSurfer refuses it)",
            );
            if g.notices.reproduce() {
                return refuse_src(g, io, c, dstlta);
            }
            invert_dst = false;
        } else {
            return refuse_src(g, io, c, dstlta);
        }
    } else if lta_mri_is_target(g, io.err, dstlta, src)? != 0 {
        invert_dst = false;
    } else if lta_mri_is_source(g, io.err, dstlta, src)? != 0 {
        invert_dst = true;
    } else {
        let _ = writeln!(
            io.out,
            "ERROR: MRIvol2volGCAM(): neither src nor dst of Dest LTA matches the mov/src vol geom"
        );
        let _ = writeln!(io.out, "mov vol geom ======================");
        print_geom(io, src);
        let _ = writeln!(io.out, "Dest LTA ======================");
        print_lta(g, io, dstlta)?;
        let _ = writeln!(
            io.out,
            "ERROR: MRIvol2volGCAM(): neither src nor dst of Dest LTA matches the mov/src vol geom"
        );
        return Ok(None);
    }
    let mut dstcopy = if invert_dst {
        let _ = writeln!(io.out, "MRIvol2volGCAM(): Inverting Dest LTA");
        lta_invert(g, io.out, io.err, InvertInto::New(dstlta))?
            .into_new()
            .expect("LTAinvert makes a new array")
    } else {
        lta_copy(g, io.out, io.err, dstlta, None)?
            .into_new()
            .expect("LTAcopy makes a new array")
    };
    if dstcopy.ltype != LINEAR_VOX_TO_VOX {
        let _ = writeln!(
            io.out,
            "MRIvol2volGCAM(): Chaning Destination LTA type to vox2vox"
        );
        let env = io.env;
        lta_change_type(g, io.out, io.err, env, &mut dstcopy, LINEAR_VOX_TO_VOX)?;
    }
    let vdst = copy(g, io, first_matrix(&dstcopy))?;
    let mut dst = mri_alloc_from_vol_geom(
        g,
        io.out,
        io.err,
        &dstcopy.xforms[0].src,
        MRI_FLOAT,
        src.nframes,
        false,
    )?;
    mri_copy_pulse_parameters(src, &mut dst);
    let dir = if vsm.is_some() {
        shift_dir(g, io, pedir)?
    } else {
        Dir::Axis(2, 1.0)
    };
    let (Some(vdst), Some(vsrc)) = (vdst, vsrc) else {
        return Err(Fatal::Exit(139));
    };
    resample(
        g,
        io,
        src,
        gcam,
        vsm,
        &vdst,
        &vsrc,
        &mut dst,
        sample_type,
        dir,
        "MRIvol2volGCAM",
    )?;
    timing(io, timer);
    Ok(Some(dst))
}

/// FS-1188's message (`reproduce`) and the case where neither end matches.
fn refuse_src(g: &mut Globals, io: &mut Io, c: &Lta, dstlta: &Lta) -> Result<Option<Mri>, Fatal> {
    let _ = writeln!(
        io.out,
        "ERROR: MRIvol2volGCAM(): neither src nor dst of Dest LTA matches the src vol geom of the Source LTA"
    );
    let _ = writeln!(io.out, "Source LTA src vol geom ======================");
    vgprint(io.out, &c.xforms[0].src, false);
    let _ = writeln!(io.out, "Dest LTA ======================");
    print_lta(g, io, dstlta)?;
    let _ = writeln!(
        io.out,
        "ERROR: MRIvol2volGCAM(): neither src nor dst of Dest LTA matches the src vol geom of the Source LTA"
    );
    Ok(None)
}

/// `printf("MRIvol2volGCAM: t=%6.4f\n", timer.seconds())`.
fn timing(io: &mut Io, timer: Instant) {
    let _ = writeln!(
        io.out,
        "MRIvol2volGCAM: t={}",
        fmt_f(timer.elapsed().as_secs_f64(), 6, 4)
    );
}

/// A geometry of 256^3 voxels of 1 mm (FreeSurfer's conformed space).
fn conformed(vg: &larmorx_freesurfer_mri::vol_geom::VolGeom) -> bool {
    vg.width == 256
        && vg.height == 256
        && vg.depth == 256
        && vg.xsize == 1.0
        && vg.ysize == 1.0
        && vg.zsize == 1.0
}

/// `MRIvol2volGCAM0(src, srclta, gcam, dstlta, vsm, sample_type, NULL)`, the older version: the
/// source LTA changed to voxel-to-voxel (`Chaning Source LTA type to vox2vox`) and inverted when
/// the source is its source; the destination LTA inverted when its source is not conformed (256^3
/// of 1 mm) and its destination is (neither: `ERROR: MRIvol2volGCAM(): neither src nor dst VG of
/// Dest LTA is conformed` and `None`, or with `MY_MORPHS_DO_NOT_CONFORM_DEAL_WITH_IT` set a
/// warning), changed to voxel-to-voxel and inverted for the loop; the output takes its
/// destination's geometry. The voxel shift is along the rows. FS-1190: no destination LTA.
#[allow(clippy::too_many_arguments)]
pub fn mri_vol2vol_gcam0(
    g: &mut Globals,
    io: &mut Io,
    src: &Mri,
    srclta: Option<&mut Lta>,
    gcam: Option<&Gcam>,
    dstlta: Option<&mut Lta>,
    vsm: Option<&Mri>,
    sample_type: i32,
) -> Result<Option<Mri>, Fatal> {
    let timer = Instant::now();
    let deal = io
        .env
        .get("MY_MORPHS_DO_NOT_CONFORM_DEAL_WITH_IT")
        .is_some();
    let _ = writeln!(
        io.out,
        "MRIvol2volGCAM0(): ==========================================================="
    );
    if deal {
        let _ = writeln!(io.out, "MY_MORPHS_DO_NOT_CONFORM_DEAL_WITH_IT is set");
    } else {
        let _ = writeln!(io.out, "MY_MORPHS_DO_NOT_CONFORM_DEAL_WITH_IT is NOT set");
    }
    if vsm.is_none() {
        let _ = writeln!(io.out, "MRIvol2volGCAM(): VSM not used");
    }
    if gcam.is_none() {
        let _ = writeln!(io.out, "MRIvol2volGCAM(): GCAM not used");
    }
    if srclta.is_none() {
        let _ = writeln!(io.out, "MRIvol2volGCAM(): Source LTA not used");
    }
    interp_line(io, sample_type);
    if let Some(v) = vsm
        && mri_dim_mismatch(src, v, false) != 0
    {
        let _ = writeln!(io.out, "ERROR: MRIvol2volGCAM(): src-vsm dim mismatch");
        return Ok(None);
    }
    let vsrc = match srclta {
        Some(sl) => {
            if sl.ltype != LINEAR_VOX_TO_VOX {
                let _ = writeln!(
                    io.out,
                    "MRIvol2volGCAM(): Chaning Source LTA type to vox2vox"
                );
                let env = io.env;
                lta_change_type(g, io.out, io.err, env, sl, LINEAR_VOX_TO_VOX)?;
            }
            if lta_mri_is_source(g, io.err, sl, src)? != 0 {
                let _ = writeln!(io.out, "MRIvol2volGCAM(): Inverting Source LTA");
                inverse(g, io, first_matrix(sl))?
            } else {
                copy(g, io, first_matrix(sl))?
            }
        }
        None => Some(matrix_identity(g, io.out, io.err, 4, None)?.unwrap_new()),
    };
    let Some(dl) = dstlta else {
        g.notices.add(
            &FS_1190,
            "MRIvol2volGCAM0: no destination LTA (FreeSurfer crashes)",
        );
        if g.notices.reproduce() {
            return Err(Fatal::Exit(139));
        }
        let _ = writeln!(
            io.out,
            "ERROR: MRIvol2volGCAM0(): a destination LTA is needed"
        );
        return Ok(None);
    };
    if !conformed(&dl.xforms[0].src) {
        if !conformed(&dl.xforms[0].dst) {
            if !deal {
                let _ = writeln!(
                    io.out,
                    "ERROR: MRIvol2volGCAM(): neither src nor dst VG of Dest LTA is conformed"
                );
                return Ok(None);
            }
            let _ = writeln!(
                io.out,
                "WARN: MRIvol2volGCAM(): neither src nor dst VG of Dest LTA is conformed"
            );
        } else {
            let _ = writeln!(io.out, "MRIvol2volGCAM(): Inverting Destination LTA");
            lta_invert(g, io.out, io.err, InvertInto::Same(dl))?;
        }
    } else {
        let _ = writeln!(io.out, "MRIvol2volGCAM(): NOT inverting Destination LTA");
    }
    if dl.ltype != LINEAR_VOX_TO_VOX {
        let _ = writeln!(
            io.out,
            "MRIvol2volGCAM(): Chaning Destination LTA type to vox2vox"
        );
        let env = io.env;
        lta_change_type(g, io.out, io.err, env, dl, LINEAR_VOX_TO_VOX)?;
    }
    let vdst = inverse(g, io, first_matrix(dl))?;
    let mut dst = mri_alloc_from_vol_geom(
        g,
        io.out,
        io.err,
        &dl.xforms[0].dst,
        MRI_FLOAT,
        src.nframes,
        false,
    )?;
    mri_copy_pulse_parameters(src, &mut dst);
    let (Some(vdst), Some(vsrc)) = (vdst, vsrc) else {
        return Err(Fatal::Exit(139));
    };
    resample(
        g,
        io,
        src,
        gcam,
        vsm,
        &vdst,
        &vsrc,
        &mut dst,
        sample_type,
        Dir::Axis(2, 1.0),
        "MRIvol2volGCAM0",
    )?;
    timing(io, timer);
    Ok(Some(dst))
}
