// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23 (fsl-topup 2203.5: applytopup.{h,cpp}). Copyright (C)
// 2009 University of Oxford (Jesper Andersson). Rust translation Copyright 2026 Karel Lopez
// Vilaret; modified from the originals. Distributed under the FSL Licence (see LICENSE).
//! applytopup: images corrected with a field topup estimated (and its movements), by Jacobian
//! modulation (`--method=jac`: each image resampled through the field, scaled by the Jacobian
//! determinant, and the images averaged) or least-squares restoration (`--method=lsr`, the
//! default: pairs of images with opposite phase encoding, each line along the phase encoding
//! restored from both by regularised least squares), with FSL's messages and exit statuses.

use std::io::Write;

use larmorx_core::defects;
use larmorx_nc_core::arma::{self, Mat};
use larmorx_nc_core::exception;
use larmorx_nc_core::interp_partial::{ExtrapParams, Interpolator};
use larmorx_nc_core::lapack::blocked;
use larmorx_nc_core::nifti::dt;
use larmorx_nc_core::options::{ArgFlag, Opt, OptionParser, Value};
use larmorx_nc_core::settings;
use larmorx_nc_core::volume::{
    self, Conversion, Extrapolation, FslError, Interpolation, SaveError, SaveOptions, SaveType,
    Volume,
};
use larmorx_nc_core::warpfns;

use crate::Console;
use crate::dispvec::DispVec;
use crate::fileio::{Datafile, TopupFileReader};
use crate::topup::Run;

const PROGRAM: &str = "applytopup";
const TITLE: &str =
    "applytopup (Version 1.0)\nCopyright(c) 2009, University of Oxford (Jesper Andersson)";
const EXAMPLES: &str = "applytopup -in=topdn,botup --topup=mytu --inindex=1,2 --out=hifi\napplytopup -in=topdn --topup=mytu --inindex=1 --method=jac --interp=spline --out=hifi\n";

/// How the program ends early.
enum End {
    /// `exit(code)`.
    Exit(u8),
    /// An exception nothing catches: `std::terminate` (SIGABRT).
    Terminate(FslError),
    /// The replica's refusal (`larmorx-nc: error: applytopup: ...`, exit 1).
    Refuse(String),
}

impl From<FslError> for End {
    fn from(e: FslError) -> End {
        End::Terminate(e)
    }
}

type R<T> = Result<T, End>;

/// `ApplyTopupException(msg)`: `what()` is `"ApplyTopup::" + msg`.
fn apply_error(msg: &str) -> FslError {
    FslError {
        printed: Vec::new(),
        type_name: "TOPUP::ApplyTopupException",
        what: format!("ApplyTopup::{msg}"),
    }
}

fn opt(
    key: &'static str,
    default: Value,
    help: &'static str,
    compulsory: bool,
    flag: ArgFlag,
) -> Opt {
    Opt {
        key,
        default,
        help,
        compulsory,
        flag,
        visible: true,
    }
}

/// The options in the order `main` adds them (`--invert` is declared but never added).
fn parser() -> OptionParser {
    use ArgFlag::{NoArgument as N, Requires};
    let r = Requires(1);
    let s = |v: &str| Value::Str(v.to_owned());
    OptionParser::new(
        TITLE,
        EXAMPLES,
        vec![
            opt(
                "-i,--imain",
                s(""),
                "comma separated list of names of input image (to be corrected)",
                true,
                r,
            ),
            opt(
                "-a,--datain",
                s(""),
                "name of text file with PE directions/times",
                true,
                r,
            ),
            opt(
                "-x,--inindex",
                s(""),
                "comma separated list of indicies into --datain of the input image (to be corrected)",
                true,
                r,
            ),
            opt(
                "-t,--topup",
                s(""),
                "name of field/movements (from topup)",
                true,
                r,
            ),
            opt(
                "-o,--out",
                s(""),
                "basename for output (warped) image",
                true,
                r,
            ),
            opt(
                "-m,--method",
                s("lsr"),
                "Use jacobian modulation (jac) or least-squares resampling (lsr), default=lsr.",
                false,
                r,
            ),
            opt(
                "-n,--interp",
                s("spline"),
                "interpolation method {trilinear,spline}, default=spline",
                false,
                r,
            ),
            opt(
                "-d,--datatype",
                s(""),
                "Force output data type [preserve char short int float double].",
                false,
                r,
            ),
            opt(
                "-v,--verbose",
                Value::Bool(false),
                "switch on diagnostic messages",
                false,
                N,
            ),
            opt(
                "-h,--help",
                Value::Bool(false),
                "display this message",
                false,
                N,
            ),
        ],
    )
}

/// The parsed command line.
struct Clp {
    imain: String,
    datain: String,
    inindex: String,
    topup: String,
    out: String,
    method: String,
    interp: Option<String>,
    datatype: Option<String>,
}

/// `applytopup [options]`: the exit status.
pub fn main(argv: &[String], run: &Run, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let con = Console::with_mode(out, err, run.mode);
    let r = match exception::catch(|| body(argv, run, &con)) {
        Ok(r) => r,
        Err(e) => Err(End::Terminate(e)),
    };
    let code = match r {
        Ok(c) => c,
        Err(End::Exit(c)) => c,
        Err(End::Terminate(e)) => {
            con.err_put(&e.printed_text());
            con.err_put(&e.terminate_text());
            con.err_line(&format!(
                "larmorx-nc: {PROGRAM} aborts here (signal 6); the replica exits with status 1"
            ));
            1
        }
        Err(End::Refuse(what)) => {
            con.err_line(&format!("larmorx-nc: error: {PROGRAM}: {what}"));
            1
        }
    };
    con.print_notices();
    code
}

fn body(argv: &[String], run: &Run, con: &Console) -> R<u8> {
    let p = parser();
    let parsed = match p.parse_command_line(argv, 0, false) {
        Ok(x) => x,
        Err((_, msg)) => {
            con.err_put(&p.usage());
            con.err_put(&format!("\n{msg}\n"));
            return Err(End::Exit(1));
        }
    };
    // "Unknown input" for words after the options is unreachable: the parser refuses them
    if parsed.get(&p, "-h,--help").bool() {
        con.err_put(&p.usage());
        return Err(End::Exit(1));
    }
    if let Some(text) = p.check_compulsory(&parsed) {
        con.err_put(&text);
        con.err_put(&p.usage());
        return Err(End::Exit(1));
    }
    let get = |k: &str| parsed.get(&p, k).str().to_owned();
    let set = |k: &str| parsed.get(&p, k).set;
    let clp = Clp {
        imain: get("-i,--imain"),
        datain: get("-a,--datain"),
        inindex: get("-x,--inindex"),
        topup: get("-t,--topup"),
        out: get("-o,--out"),
        method: get("-m,--method"),
        interp: set("-n,--interp").then(|| get("-n,--interp")),
        datatype: set("-d,--datatype").then(|| get("-d,--datatype")),
    };
    applytopup(&clp, run, con)
}

/// `parse_commaseparated_list(list)`: the pieces between commas (empty ones kept).
fn comma_list(list: &str) -> Vec<String> {
    list.split(',').map(str::to_owned).collect()
}

/// `parse_commaseparated_numbers(list)`: `atoi` of each piece, as `unsigned int`.
fn comma_numbers(list: &str) -> Vec<u32> {
    comma_list(list)
        .iter()
        .map(|s| settings::atoi(s) as u32)
        .collect()
}

/// The lines FSL printed on standard error while a step ran.
fn print_lines(con: &Console, lines: &mut Vec<String>) {
    for l in lines.drain(..) {
        con.err_line(&l);
    }
}

fn applytopup(clp: &Clp, run: &Run, con: &Console) -> R<u8> {
    let fsl = run.fsl;
    let interp = match clp.interp.as_deref() {
        None | Some("spline") => Interpolation::Spline,
        Some("trilinear") => Interpolation::Trilinear,
        Some(other) => {
            return Err(apply_error(&format!("Unknown interpolation type {other}")).into());
        }
    };
    let infnames = comma_list(&clp.imain);
    let inindices = comma_numbers(&clp.inindex);
    if infnames.len() != inindices.len() {
        return Err(apply_error("Mismatched --in and --inindex lists").into());
    }
    let mut dtypecode = dt::FLOAT;
    if let Some(d) = clp.datatype.as_deref() {
        dtypecode = match d {
            "preserve" => volume::file_dtype(&infnames[0], fsl)?,
            "char" => dt::UNSIGNED_CHAR,
            "short" => dt::SIGNED_SHORT,
            "int" => dt::SIGNED_INT,
            "float" => dt::FLOAT,
            "double" => dt::DOUBLE,
            other => return Err(apply_error(&format!("Unknown data type {other}")).into()),
        };
    }
    let mut scans: Vec<Volume> = Vec::with_capacity(infnames.len());
    for (i, name) in infnames.iter().enumerate() {
        let (v, _, _) = volume::read_volume4d::<f32>(name, fsl)?;
        if i > 0 && !(0..7).all(|k| v.dims[k] == scans[0].dims[k]) {
            return Err(apply_error(&format!(
                "Mismatched input files {name} and {}",
                infnames[0]
            ))
            .into());
        }
        scans.push(v);
    }
    let mut lines = Vec::new();
    let datafile = Datafile::read(&clp.datain, &mut lines);
    print_lines(con, &mut lines);
    let datafile = datafile?;
    if inindices
        .iter()
        .any(|&i| i == 0 || i as usize > datafile.n())
    {
        return Err(apply_error("Invalid entry in --inindex list").into());
    }
    let topupfile = TopupFileReader::read(&clp.topup, fsl, &mut lines);
    print_lines(con, &mut lines);
    let mut topupfile = topupfile?;
    let s0 = &scans[0];
    let mut mask = Volume::<i8>::new(s0.xsize(), s0.ysize(), s0.zsize(), 1);
    mask.copy_properties(s0);
    mask.data.fill(1);
    let field_shape = field_shape(&mut topupfile)?;
    let image_shape = [s0.xsize(), s0.ysize(), s0.zsize()];
    let ctx = Ctx {
        datafile: &datafile,
        inindices: &inindices,
        interp,
        nthr: run.threads.max(1) as u32,
        con,
    };
    let mut ovol = match clp.method.as_str() {
        "jac" => {
            shape_check(con, field_shape, image_shape, false)?;
            jac_resample(&ctx, &mut topupfile, &mut mask, &mut scans)?
        }
        "lsr" => {
            shape_check(con, field_shape, image_shape, true)?;
            if !topupfile.has_moves() {
                // FSL-135: MoveMatrix's index error ends the program
                if !con.reproduce() {
                    con.notice(
                        &defects::FSL_135,
                        "--method=lsr without movements: a clear message",
                    );
                    return Err(End::Refuse(format!(
                        "--method=lsr needs topup's movement parameters ({}_movpar.txt): \
                         --topup={} gives none (use --method=jac, which takes none as no \
                         movement) (FSL-135)",
                        clp.topup, clp.topup
                    )));
                }
                con.notice(
                    &defects::FSL_135,
                    "--method=lsr without movements: FSL's internal index error",
                );
            }
            resample_using_movement_parameters(&ctx, &topupfile, &mut mask, &mut scans)?;
            lsr_resample(&ctx, &mut topupfile, &mask, &scans)?
        }
        m @ ("vb2D" | "vb3D" | "vb4D") => {
            return Err(End::Refuse(format!(
                "--method={m} (an undocumented variational-Bayes restoration) is not \
                 replicated yet"
            )));
        }
        other => {
            // FSL-135 (rule B): FSL writes nothing and exits 0
            if !con.reproduce() {
                con.notice(&defects::FSL_135, "an unknown --method: a clear error");
                return Err(End::Refuse(format!(
                    "--method={other}: unknown method (jac or lsr); FSL writes no output and \
                     exits 0 (FSL-135)"
                )));
            }
            con.notice(
                &defects::FSL_135,
                "an unknown --method: no output written, as in applytopup",
            );
            Volume::new7([0, 0, 0, 0, 1, 1, 1])
        }
    };
    let n3 = (ovol.xsize() * ovol.ysize() * ovol.zsize()) as usize;
    for t in 0..ovol.tsize().max(0) as usize {
        for (i, &m) in mask.data.iter().enumerate().take(n3) {
            if m == 0 {
                ovol.data[t * n3 + i] = 0.0;
            }
        }
    }
    ovol.display_max = 0.0;
    ovol.display_min = 0.0;
    save_output(
        &ovol,
        &clp.out,
        clp.datatype.is_some().then_some(dtypecode),
        run,
        con,
    )?;
    Ok(0)
}

/// What the methods share.
struct Ctx<'a, 'c> {
    datafile: &'a Datafile,
    inindices: &'a [u32],
    interp: Interpolation,
    nthr: u32,
    con: &'a Console<'c>,
}

impl Ctx<'_, '_> {
    fn pe(&self, index: u32) -> [f64; 3] {
        self.datafile.pv[index as usize - 1]
    }

    fn rt(&self, index: u32) -> f64 {
        self.datafile.rt[index as usize - 1]
    }

    /// FSL-132: newimage keeps each volume of a series with the properties the series had when
    /// the volume was first used; the first image's first volume is first used (for the mask's
    /// size) before the interpolation is set, so FSL resamples it with the interpolation it was
    /// read with. Reproduced, `vin` (the first image's first volume) gets `stale`.
    fn first_volume(&self, vin: &mut Volume, i: usize, j: usize, stale: Interpolation) {
        if i != 0 || j != 0 || stale == self.interp {
            return;
        }
        if self.con.reproduce() {
            self.con.notice(
                &defects::FSL_132,
                "the first image's first volume resampled by trilinear interpolation, as in \
                 applytopup",
            );
            vin.interp = stale;
        } else {
            self.con.notice(
                &defects::FSL_132,
                "the first image's first volume resampled by splines, as every other volume",
            );
        }
    }

    /// The scans' interpolation (`setinterpolationmethod`, spline order 3), periodic
    /// extrapolation, valid along the phase encoding when it is one of x and y.
    fn prepare_scan(&self, scan: &mut Volume, index: u32) {
        scan.interp = self.interp;
        if self.interp == Interpolation::Spline {
            scan.splineorder = 3;
        }
        scan.extrap = Extrapolation::Periodic;
        let pe = self.pe(index);
        scan.ep_valid = if pe[0] != 0.0 && pe[1] == 0.0 {
            [true, false, false]
        } else if pe[0] == 0.0 && pe[1] != 0.0 {
            [false, true, false]
        } else {
            [false, false, false]
        };
    }
}

/// The field's grid.
fn field_shape(t: &mut TopupFileReader) -> R<[i64; 3]> {
    let f = t.field_as_volume()?;
    Ok([f.xsize(), f.ysize(), f.zsize()])
}

/// FSL-134: a field whose grid is not the images'.
fn shape_check(con: &Console, field: [i64; 3], image: [i64; 3], lsr: bool) -> R<()> {
    if field == image {
        return Ok(());
    }
    let what = format!(
        "the field (--topup) is {} x {} x {} voxels, the images {} x {} x {}",
        field[0], field[1], field[2], image[0], image[1], image[2]
    );
    if lsr {
        // FSL reads the field's lines past its end (undefined behaviour): never reproduced
        con.notice(&defects::FSL_134, "a field of another grid: refused");
        return Err(End::Refuse(format!(
            "{what}: --method=lsr reads lines of the field outside it (FSL-134)"
        )));
    }
    if !con.reproduce() {
        con.notice(&defects::FSL_134, "a field of another grid: refused");
        return Err(End::Refuse(format!(
            "{what}: the field's values would be laid on the images' grid as they are stored \
             (FSL-134)"
        )));
    }
    con.notice(
        &defects::FSL_134,
        "a field of another grid: its values laid on the images' grid as stored, as in \
         applytopup",
    );
    Ok(())
}

/// `jac_resample`: each scan resampled through the field (and its movements), times the
/// Jacobian determinant of the displacement; the scans averaged.
fn jac_resample(
    ctx: &Ctx,
    topupfile: &mut TopupFileReader,
    mask: &mut Volume<i8>,
    scans: &mut [Volume],
) -> R<Volume> {
    // the first volume's interpolation as read (FSL-132)
    let stale = scans[0].interp;
    let mut tmp = scans[0].sub_volume(0)?;
    let (x, y, z) = (tmp.xsize(), tmp.ysize(), tmp.zsize());
    let n3 = (x * y * z) as usize;
    let mut df = Volume::new(x, y, z, 3);
    df.copy_properties(&tmp);
    let mut ovol = scans[0].clone();
    ovol.data.fill(0.0);
    let (xdim, ydim) = (f64::from(ovol.xdim()), f64::from(ovol.ydim()));
    let nt = scans[0].tsize() as usize;
    for i in 0..scans.len() {
        let index = ctx.inindices[i];
        ctx.prepare_scan(&mut scans[i], index);
        let (pe, rt) = (ctx.pe(index), ctx.rt(index));
        let field = topupfile.field_as_volume()?;
        // df[0] = field: copydata needs the same number of voxels
        if field.data.len() != n3 {
            return Err(
                FslError::imthrow("Attempted to copydata with non-matching sizes", 2).into(),
            );
        }
        let k1 = (pe[1] * rt * ydim) as f32;
        let k0 = (pe[0] * rt * xdim) as f32;
        for v in 0..n3 {
            df.data[n3 + v] = k1 * field.data[v];
            df.data[v] = field.data[v] * k0;
            df.data[2 * n3 + v] = 0.0;
        }
        let mut jac = tmp.clone();
        jac.data.fill(0.0);
        match topupfile.field() {
            Ok(f) => {
                let mut xc = f.clone();
                let mut yc = f.clone();
                let mut zc = f;
                xc.scale_field(pe[0] * rt * xdim);
                yc.scale_field(pe[1] * rt * ydim);
                zc.scale_field(0.0);
                warpfns::deffield2jacobian(&mut xc, &mut yc, &mut zc, &mut jac)?;
            }
            Err(_) => {
                // a field image (--fout, fugue's): the Jacobian from its interpolated
                // derivatives
                let mut xc = field.clone();
                let mut yc = field.clone();
                let mut zc = field;
                let kx = (pe[0] * rt * xdim) as f32;
                // FSL-134 (rule A): FSL scales the y displacement by the x voxel size
                let ky_fsl = (pe[1] * rt * xdim) as f32;
                let ky_fix = (pe[1] * rt * ydim) as f32;
                if ky_fsl != ky_fix && pe[1] != 0.0 {
                    ctx.con.notice(
                        &defects::FSL_134,
                        if ctx.con.reproduce() {
                            "the field image's y displacement in units of the x voxel size, as \
                             in applytopup"
                        } else {
                            "the field image's y displacement in units of the y voxel size"
                        },
                    );
                }
                let ky = if ctx.con.reproduce() { ky_fsl } else { ky_fix };
                for v in xc.data.iter_mut() {
                    *v *= kx;
                }
                for v in yc.data.iter_mut() {
                    *v *= ky;
                }
                zc.data.fill(0.0);
                volume_deffield_2_jacobian(&xc, &yc, &zc, &mut jac)?;
            }
        }
        let m = if topupfile.move_par_valid() {
            topupfile.move_matrix(index)?
        } else {
            Mat::eye(4)
        };
        let mut tmpmask = mask.clone();
        for j in 0..nt {
            tmp.data.fill(0.0);
            tmpmask.data.fill(0);
            let mut vin = scans[i].sub_volume(j as i64)?;
            ctx.first_volume(&mut vin, i, j, stale);
            warpfns::general_transform(&vin, &m, &df, &mut tmp, Some(&mut tmpmask), ctx.nthr)?;
            for (a, &b) in mask.data.iter_mut().zip(&tmpmask.data) {
                *a = (i32::from(*a) * i32::from(b)) as i8;
            }
            let o = &mut ovol.data[j * n3..(j + 1) * n3];
            for v in 0..n3 {
                o[v] += tmp.data[v] * jac.data[v];
            }
        }
    }
    let k = scans.len() as f32;
    for v in ovol.data.iter_mut() {
        *v /= k;
    }
    Ok(ovol)
}

/// `volume_deffield_2_jacobian(pxd, pyd, pzd, jac)`: the Jacobian determinant of the
/// displacements (mm) from the spline interpolation's derivatives of each, at every voxel of
/// `jac` (FSL builds its size check's exception but never throws it).
fn volume_deffield_2_jacobian(
    pxd: &Volume,
    pyd: &Volume,
    pzd: &Volume,
    jac: &mut Volume,
) -> Result<(), FslError> {
    let per = |p: &Volume, d: f32| {
        let mut v = p.clone();
        for x in v.data.iter_mut() {
            *x /= d;
        }
        v.interp = Interpolation::Spline;
        v
    };
    let xd = per(pxd, pxd.xdim());
    let yd = per(pyd, pyd.ydim());
    let zd = per(pzd, pzd.zdim());
    let (ix, iy, iz) = (
        Interpolator::new(&xd)?,
        Interpolator::new(&yd)?,
        Interpolator::new(&zd)?,
    );
    let ep = ExtrapParams::DEFAULT;
    for k in 0..jac.zsize() {
        for j in 0..jac.ysize() {
            for i in 0..jac.xsize() {
                let (x, y, z) = (i as f32, j as f32, k as f32);
                let (_, [dxdx, dxdy, dxdz]) = ix.interp3partial(x, y, z, &ep)?;
                let (_, [dydx, dydy, dydz]) = iy.interp3partial(x, y, z, &ep)?;
                let (_, [dzdx, dzdy, dzdz]) = iz.interp3partial(x, y, z, &ep)?;
                let f = f64::from;
                let first = (1.0 + f(dxdx)) * (1.0 + f(dydy)) * (1.0 + f(dzdz))
                    + f(dxdz * dydx * dzdy)
                    + f(dxdy * dydz * dzdx);
                let v = first as f32;
                let second = f(dxdz) * (1.0 + f(dydy)) * f(dzdx)
                    + (1.0 + f(dxdx)) * f(dydz) * f(dzdy)
                    + f(dxdy * dydx) * (1.0 + f(dzdz));
                let w = jac.idx(i, j, k);
                jac.data[w] = (f(v) - second) as f32;
            }
        }
    }
    Ok(())
}

/// `resample_using_movement_parameters`: each scan resampled by its movement (a rigid-body
/// matrix), in place; the mask keeps the voxels every resampling reached.
fn resample_using_movement_parameters(
    ctx: &Ctx,
    topupfile: &TopupFileReader,
    mask: &mut Volume<i8>,
    scans: &mut [Volume],
) -> R<()> {
    // the first volume's interpolation as read (FSL-132)
    let stale = scans[0].interp;
    let mut tmpmask = mask.clone();
    let mut tmp = scans[0].sub_volume(0)?;
    tmp.data.fill(0.0);
    let n3 = tmp.data.len();
    let nt = scans[0].tsize() as usize;
    for i in 0..scans.len() {
        let index = ctx.inindices[i];
        ctx.prepare_scan(&mut scans[i], index);
        let m = topupfile.move_matrix(index)?;
        for j in 0..nt {
            tmp.data.fill(0.0);
            tmpmask.data.fill(0);
            let mut vin = scans[i].sub_volume(j as i64)?;
            ctx.first_volume(&mut vin, i, j, stale);
            warpfns::affine_transform(&vin, &m, &mut tmp, Some(&mut tmpmask), ctx.nthr)?;
            for (a, &b) in mask.data.iter_mut().zip(&tmpmask.data) {
                *a = (i32::from(*a) * i32::from(b)) as i8;
            }
            scans[i].data[j * n3..(j + 1) * n3].copy_from_slice(&tmp.data);
        }
    }
    Ok(())
}

/// `TopupCollections`: the scans grouped by phase-encoding axis (x first, then y), each group
/// needing two different displacement scales.
struct Collections {
    /// Per collection: (index into the scans, index into --datain).
    groups: Vec<Vec<(usize, u32)>>,
}

impl Collections {
    fn new(indx: &[u32], d: &Datafile) -> Result<Collections, FslError> {
        let pe = |i: u32| d.pv[i as usize - 1];
        let rt = |i: u32| d.rt[i as usize - 1];
        let mut tmp: [Vec<usize>; 2] = [Vec::new(), Vec::new()];
        for (i, &ix) in indx.iter().enumerate() {
            let p = pe(ix);
            if p[0] != 0.0 && p[1] == 0.0 {
                tmp[0].push(i);
            } else if p[0] == 0.0 && p[1] != 0.0 {
                tmp[1].push(i);
            } else {
                return Err(apply_error(
                    "Invalid phase-encode vector for least-squares restoration",
                ));
            }
        }
        let mut unique = [false, false];
        for a in 0..2 {
            for j in 1..tmp[a].len() {
                let s = |k: usize| pe(indx[tmp[a][k]])[a] * rt(indx[tmp[a][k]]);
                if s(j) != s(0) {
                    unique[a] = true;
                }
            }
        }
        if (!tmp[0].is_empty() && !unique[0]) || (!tmp[1].is_empty() && !unique[1]) {
            return Err(apply_error(
                "Invalid combination of phase-encode vectors for least-squares restoration",
            ));
        }
        let groups = tmp
            .iter()
            .filter(|g| !g.is_empty())
            .map(|g| g.iter().map(|&s| (s, indx[s])).collect())
            .collect();
        Ok(Collections { groups })
    }
}

/// Armadillo's `band_helper::is_band(KL, KU, A, 32)`: whether `solve()` would take its band
/// solver for the square matrix `a` (the corners zero and few enough diagonals).
fn is_band(a: &Mat) -> bool {
    let n = a.rows();
    if n < 32 {
        return false;
    }
    let at = |r: usize, c: usize| a.at(r, c);
    if at(n - 2, 0) != 0.0 || at(n - 1, 0) != 0.0 || at(n - 2, 1) != 0.0 || at(n - 1, 1) != 0.0 {
        return false;
    }
    if at(0, n - 2) != 0.0 || at(1, n - 2) != 0.0 || at(0, n - 1) != 0.0 || at(1, n - 1) != 0.0 {
        return false;
    }
    let threshold = (n * n) / 4;
    let (mut kl, mut ku) = (0usize, 0usize);
    for col in 0..n {
        let mut first = col;
        let mut last = col;
        for row in 0..col {
            if at(row, col) != 0.0 {
                first = row;
                break;
            }
        }
        for row in col + 1..n {
            if at(row, col) != 0.0 {
                last = row;
            }
        }
        let (lc, uc) = (last - col, col - first);
        if lc > kl || uc > ku {
            kl = kl.max(lc);
            ku = ku.max(uc);
            let nnz = n * (kl + ku + 1) - (kl * (kl + 1) + ku * (ku + 1)) / 2;
            if nnz > threshold {
                return false;
            }
        }
    }
    true
}

/// The 1-norm (largest column sum of magnitudes); NaN propagates.
fn norm1(a: &Mat) -> f64 {
    let mut m = 0.0f64;
    for c in 0..a.cols() {
        let s: f64 = (0..a.rows()).map(|r| a.at(r, c).abs()).sum();
        if s.is_nan() {
            return f64::NAN;
        }
        m = m.max(s);
    }
    m
}

/// `CroutMatrix XtX = KtK; XtX.i()`: armawrap's LU (`arma::lu`: LAPACK `dgetrf`, then `L`
/// with its unit diagonal and `U` apart, `P` from the pivots) and the inverse its constructor
/// computes, `U⁻¹ (L⁻¹ (P I))`, by two `solve()` calls on triangular matrices (LAPACK
/// `dtrtrs`: OpenBLAS's triangular kernels, the same as `dgetrs`'s on the identity). `Err`
/// where Armadillo would take another solver (a band matrix) or its approximate solution (a
/// condition estimate below machine precision), neither replicated.
fn crout_inverse(ktk: &Mat) -> Result<Mat, String> {
    let n = ktk.rows();
    if n > 256 {
        return Err(format!(
            "lines of {n} voxels along the phase encoding: OpenBLAS's LU of more than 256 rows \
             is not replicated yet"
        ));
    }
    if n < 8 {
        return Err(format!(
            "lines of {n} voxels along the phase encoding: Armadillo's solvers for systems this \
             small are not replicated"
        ));
    }
    let mut lu = ktk.as_slice().to_vec();
    let mut ipiv = vec![0i32; n];
    let _ = blocked::getrf_any(&mut lu, n, n, n, &mut ipiv);
    let lum = Mat::from_col_major(n, n, &lu);
    let mut l = Mat::zeros(n, n);
    let mut u = Mat::zeros(n, n);
    for c in 0..n {
        for r in 0..n {
            if r > c {
                l.set(r, c, lum.at(r, c));
            } else {
                u.set(r, c, lum.at(r, c));
            }
        }
        l.set(c, c, 1.0);
    }
    if is_band(&l) || is_band(&u) {
        return Err(
            "the field gives a band system on this line (it is zero at both ends of the line): \
             Armadillo's band solver is not replicated yet"
                .to_owned(),
        );
    }
    let mut b = Mat::eye(n);
    blocked::getrs_any(&lu, n, n, &ipiv, b.as_mut_slice(), n, n);
    // dtrcon's estimates are at least the true reciprocal condition numbers, which are at
    // least 1 / (|L| |U| |inverse|): far above machine precision, Armadillo keeps the solution
    let g = 1.0 / (norm1(&l) * norm1(&u) * norm1(&b));
    if g.is_nan() || g <= 1e-12 {
        return Err(
            "a near-singular system on this line: Armadillo's approximate solution is not \
             replicated"
                .to_owned(),
        );
    }
    Ok(b)
}

/// `lsr_resample`: every line along the phase encoding restored from the scans of its
/// collection by regularised least squares (`(KᵀK + 0.01 SᵀS)⁻¹ Kᵀ y`); lines where the mask
/// misses a voxel are zero.
fn lsr_resample(
    ctx: &Ctx,
    topupfile: &mut TopupFileReader,
    mask: &Volume<i8>,
    scans: &[Volume],
) -> R<Volume> {
    let field = topupfile.field_as_volume()?;
    let collections = Collections::new(ctx.inindices, ctx.datafile)?;
    let mut ovol = scans[0].clone();
    ovol.data.fill(0.0);
    let (xs, ys, zs) = (scans[0].xsize(), scans[0].ysize(), scans[0].zsize());
    let nt = scans[0].tsize() as usize;
    let n3 = (xs * ys * zs) as usize;
    let mut mis_maps = Vec::new();
    let reproduce = ctx.con.reproduce();
    let mut wrap_differs = false;
    for g in &collections.groups {
        let row = ctx.pe(g[0].1)[0] != 0.0;
        let sz = if row { xs } else { ys } as usize;
        let nlines = if row { ys } else { xs };
        let mut mis_map = vec![false; (zs * nlines) as usize];
        let mut dv = DispVec::new(sz, reproduce);
        let ns = g.len();
        let mut k = Mat::zeros(ns * sz, sz);
        let mut yv = Mat::zeros(ns * sz, nt);
        let sts = arma::syrk_t(&dv.s_matrix(false));
        for kk in 0..zs {
            for ij in 0..nlines {
                let ok = if row {
                    (0..xs).all(|i| mask.value(i, ij, kk) != 0)
                } else {
                    (0..ys).all(|j| mask.value(ij, j, kk) != 0)
                };
                if !ok {
                    mis_map[(kk * nlines + ij) as usize] = true;
                    continue;
                }
                if row {
                    dv.set_from_row(&field, kk, ij);
                } else {
                    dv.set_from_column(&field, kk, ij);
                }
                for (a, &(_, index)) in g.iter().enumerate() {
                    let axis = if row { 0 } else { 1 };
                    let sf = ctx.pe(index)[axis] * ctx.rt(index);
                    let km = dv.k_matrix(sf)?;
                    k.set_sub(a * sz, 0, &km);
                }
                wrap_differs |= dv.ca.wrapped_differently;
                dv.ca.wrapped_differently = false;
                let ktk0 = arma::syrk_t(&k);
                let mut ktk = Mat::zeros(sz, sz);
                for e in 0..sz * sz {
                    ktk.put(e, ktk0.get(e) + sts.get(e) * 0.01);
                }
                let ai = crout_inverse(&ktk).map_err(End::Refuse)?;
                for d in 0..nt {
                    for (a, &(s, _)) in g.iter().enumerate() {
                        let base = d * n3;
                        for p in 0..sz {
                            let v = if row {
                                scans[s].data[base + scans[s].idx(p as i64, ij, kk)]
                            } else {
                                scans[s].data[base + scans[s].idx(ij, p as i64, kk)]
                            };
                            yv.set(a * sz + p, d, f64::from(v));
                        }
                    }
                }
                let kty = arma::mul(&k, &yv, true);
                let bm = arma::mul(&ai, &kty, false);
                for d in 0..nt {
                    for p in 0..sz {
                        let w = if row {
                            ovol.idx(p as i64, ij, kk)
                        } else {
                            ovol.idx(ij, p as i64, kk)
                        };
                        let o = &mut ovol.data[d * n3 + w];
                        *o = (f64::from(*o) + bm.at(p, d)) as f32;
                    }
                }
            }
        }
        mis_maps.push((row, nlines, mis_map));
    }
    if wrap_differs {
        ctx.con.notice(
            &defects::FSL_133,
            if reproduce {
                "the displacements before a line's first voxel read where applytopup's unsigned \
                 remainder puts them"
            } else {
                "the displacements before a line's first voxel read circularly (from the line's \
                 end)"
            },
        );
    }
    for (row, nlines, mis_map) in mis_maps {
        for d in 0..nt {
            for kk in 0..zs {
                for ij in 0..nlines {
                    if !mis_map[(kk * nlines + ij) as usize] {
                        continue;
                    }
                    if row {
                        for i in 0..xs {
                            let w = ovol.idx(i, ij, kk);
                            ovol.data[d * n3 + w] = 0.0;
                        }
                    } else {
                        for j in 0..ys {
                            let w = ovol.idx(ij, j, kk);
                            ovol.data[d * n3 + w] = 0.0;
                        }
                    }
                }
            }
        }
    }
    Ok(ovol)
}

/// `save_volume(ovol, out)` (float), or `save_volume_dtype(ovol, out, datatype)` with
/// FSL-09's integer conversion corrected unless reproduced.
fn save_output(v: &Volume, name: &str, datatype: Option<i16>, run: &Run, con: &Console) -> R<()> {
    let opts =
        SaveOptions::new(volume::BUILDSTRING.as_bytes()).reproducing_offset(run.mode.reproduce());
    if volume::offset_defect_applies(v, run.fsl) {
        con.notice(
            &defects::FSL_56,
            &format!(
                "{} NIfTI extensions of different sizes: the data offset counts each \
                 extension's own size (NewNifti adds the first one's once per extension)",
                v.extensions.len()
            ),
        );
    }
    let saved = match datatype {
        None => v.save_volume(name, run.fsl, &opts).map(|_| ()),
        Some(datatype) => {
            let ty = match volume::save_type_for(datatype) {
                Ok(t) => t,
                Err(msg) => {
                    con.err_line(&msg);
                    con.err_line(&format!(
                        "NEWIMAGE::save_volume_dtype: DT {datatype} not supported: Success"
                    ));
                    return Ok(());
                }
            };
            let integer = matches!(ty, SaveType::Char | SaveType::Short | SaveType::Int);
            let (lo, hi) = match ty {
                SaveType::Char => (0.0, 255.0),
                SaveType::Short => (-32768.0, 32767.0),
                _ => (f64::from(i32::MIN), f64::from(i32::MAX)),
            };
            let rounds = integer
                && v.data.iter().any(|x| {
                    x.round_ties_even() != x.trunc()
                        || !x.is_finite()
                        || f64::from(x.round_ties_even()) < lo
                        || f64::from(x.round_ties_even()) > hi
                });
            let (ty, conv, code) = if run.mode.fix() {
                if datatype == dt::INT8 {
                    con.notice(
                        &defects::FSL_09,
                        "int8 data are written as int8, rounded and clamped; applytopup writes \
                         uint8 and truncates",
                    );
                    (SaveType::Int8, Conversion::RoundClamp, Some(dt::INT8))
                } else if integer {
                    if rounds {
                        con.notice(
                            &defects::FSL_09,
                            "integer outputs are rounded to the nearest and clamped; applytopup \
                             truncates toward zero and wraps out-of-range values",
                        );
                    }
                    (ty, Conversion::RoundClamp, None)
                } else {
                    (ty, Conversion::X86Cast, None)
                }
            } else {
                if datatype == dt::INT8 {
                    con.notice(
                        &defects::FSL_09,
                        "int8 data are written as uint8, as in applytopup",
                    );
                } else if rounds {
                    con.notice(
                        &defects::FSL_09,
                        "integer outputs are truncated toward zero, as in applytopup",
                    );
                }
                (ty, Conversion::X86Cast, None)
            };
            v.save_typed(name, ty, conv, run.fsl, &opts, code)
        }
    };
    match saved {
        Ok(()) => Ok(()),
        Err(SaveError::OutputType(e)) => {
            for l in &e.printed {
                con.err_line(l);
            }
            Err(End::Exit(1))
        }
        Err(SaveError::Exception(e)) => Err(e.into()),
    }
}
