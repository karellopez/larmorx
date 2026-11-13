// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Control points (`utils/ctrpoints.cpp`, `include/ctrpoints.h`; item 3.1-33):
//! `MRIreadControlPoints`, `MRIwriteControlPoints`, `MRIfillPoints`.

use std::io::Write;
use std::path::Path;

use larmorx_freesurfer_core::cscan::{Bytes, Scanf};
use larmorx_freesurfer_core::cstd::{fmt_f, format_double};
use larmorx_freesurfer_core::diag::DIAG_SHOW;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_NO_FILE, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, matrix_alloc, matrix_multiply};
use larmorx_freesurfer_core::matrix_ops::matrix_inverse;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::sse::{add64, div64, mul64, sub64};
use larmorx_freesurfer_core::system;
use larmorx_freesurfer_core::timer;
use larmorx_freesurfer_core::utils;
use larmorx_freesurfer_mri::mri::Mri;
use larmorx_freesurfer_mri::tkreg::xfm_crs2xyz_tkreg;
use larmorx_freesurfer_mri::vol_geom::xfm_crs2xyz;
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

use crate::cfile::CFile;
use crate::label::{self, LABEL_COORDS_SCANNER_RAS};

/// `MPoint`: a control point.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MPoint {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// `MRIreadControlPoints`' outputs: the caller's `count` and `useRealRAS` (each kept until the
/// reader sets it, as FreeSurfer's pointers are).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ControlInfo {
    pub count: i32,
    pub use_real_ras: i32,
}

/// The values one `sscanf` call of `"%f %f %f"` assigned.
fn three_floats(errno: &mut i32, line: &[u8]) -> (i32, [f32; 3]) {
    let mut b = Bytes::new(line);
    let mut sc = Scanf::new(&mut b);
    let mut v = [0f32; 3];
    for (k, x) in v.iter_mut().enumerate() {
        if k > 0 {
            sc.text(errno, b" ");
        }
        *x = sc.f(errno, true).unwrap_or(0.0);
    }
    (sc.finish(errno), v)
}

/// `MRIreadControlPoints(fname, &count, &useRealRAS)`. A name whose extension is `label` (any
/// case): the label file ([`label::label_read`] without a subject; `MRIreadControlPoints: could
/// not load label file <name>` as an error and `None` when it does not read), its points' first
/// `n_points` coordinates, `count` its point count and `useRealRAS` 1 for scanner coordinates.
/// Any other name: `useRealRAS` 0; with `DIAG_SHOW`, `reading control points from <name>...`;
/// a file that does not open is an error (`cannot open control point file`, `ERROR_BADPARM`) and
/// `None`. Its lines (read with `fgetl` and a 199-byte buffer: comments and blank lines skipped,
/// cut at `#`) that hold three numbers (`%f %f %f`) are the points; a line `useRealRAS <n>` (two
/// items read by `%s %d`) sets `useRealRAS`; `Reading <k> control points...` on standard error.
#[allow(clippy::too_many_arguments)]
pub fn mri_read_control_points(
    g: &mut Globals,
    env: &Environment,
    out: &mut dyn Write,
    err: &mut dyn Write,
    dir: Option<&Path>,
    fname: &str,
    info: &mut ControlInfo,
) -> Result<Option<Vec<MPoint>>, Fatal> {
    let ext = utils::file_name_extension(g, fname);
    if utils::stricmp(g, &ext, "label") == 0 {
        let Some(area) = label::label_read(g, env, out, err, dir, None, fname)? else {
            let m = format!("MRIreadControlPoints: could not load label file {fname}");
            log::error_printf(g, err, ERROR_NO_FILE, &m);
            return Ok(None);
        };
        info.count = area.n_points;
        info.use_real_ras = i32::from(area.coords == LABEL_COORDS_SCANNER_RAS);
        let n = (area.n_points.max(0) as usize).min(area.lv.len());
        return Ok(Some(
            area.lv[..n]
                .iter()
                .map(|v| MPoint {
                    x: f64::from(v.x),
                    y: f64::from(v.y),
                    z: f64::from(v.z),
                })
                .collect(),
        ));
    }
    info.use_real_ras = 0;
    if g.gdiag & DIAG_SHOW != 0 {
        let _ = writeln!(err, "reading control points from {fname}...");
    }
    let mut fp = match CFile::open(dir, fname, "r") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            log::error_printf(g, err, ERROR_BADPARM, "cannot open control point file");
            return Ok(None);
        }
    };
    let mut num = 0i32;
    while let Some(cp) = label::fgetl(g, &mut fp, 199) {
        let mut errno = g.errno;
        let (i, _) = three_floats(&mut errno, &cp);
        if i == 3 {
            num += 1;
        } else {
            let mut b = Bytes::new(&cp);
            let mut sc = Scanf::new(&mut b);
            let text = sc.s(&mut errno, usize::MAX, true);
            sc.text(&mut errno, b" ");
            let val = sc.d(&mut errno, true);
            let i = sc.finish(&mut errno);
            if i == 2 && text.as_deref() == Some(b"useRealRAS") {
                info.use_real_ras = val.unwrap_or(0);
            }
        }
        g.errno = errno;
    }
    let _ = writeln!(err, "Reading {num} control points...");
    info.count = num;
    fp.rewind();
    let mut points = Vec::with_capacity(num.max(0) as usize);
    while (points.len() as i32) < num {
        let Some(cp) = label::fgetl(g, &mut fp, 199) else {
            break;
        };
        let mut errno = g.errno;
        let (n, [x, y, z]) = three_floats(&mut errno, &cp);
        g.errno = errno;
        if n == 3 {
            points.push(MPoint {
                x: f64::from(x),
                y: f64::from(y),
                z: f64::from(z),
            });
        }
    }
    fp.close();
    Ok(Some(points))
}

/// `MRIwriteControlPoints(points, count, useRealRAS, fname)`: with `DIAG_SHOW`, `Writing control
/// points to <name>...` on standard error; a `useRealRAS` other than 0 or 1 is an error
/// (`MRIwriteControlPoints useRealRAS must be 0 (surfaceRAS) or 1 (scannerRAS) but <n>` and a
/// newline) and gives `ERROR_BADPARM`, as does a file that does not open
/// (`MRIwriteControlPoints(<name>): could not open file`) or a point that does not write. The
/// file: each point `%f %f %f`, then `info`, `numpoints <count>`, `useRealRAS <n>` and `written
/// by <user> on <date>` (the effective user's name; `currentDateTime()`, which
/// `FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING` replaces). `NO_ERROR`.
#[allow(clippy::too_many_arguments)]
pub fn mri_write_control_points(
    g: &mut Globals,
    env: &Environment,
    err: &mut dyn Write,
    dir: Option<&Path>,
    points: &[MPoint],
    count: i32,
    use_real_ras: i32,
    fname: &str,
) -> i32 {
    if g.gdiag & DIAG_SHOW != 0 {
        let _ = writeln!(err, "Writing control points to {fname}...");
    }
    if !(0..=1).contains(&use_real_ras) {
        let m = format!(
            "MRIwriteControlPoints useRealRAS must be 0 (surfaceRAS) or 1 (scannerRAS) but {use_real_ras}\n"
        );
        log::error_printf(g, err, ERROR_BADPARM, &m);
        return ERROR_BADPARM;
    }
    let mut fp = match CFile::open(dir, fname, "w") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            let m = format!("MRIwriteControlPoints({fname}): could not open file");
            log::error_printf(g, err, ERROR_BADPARM, &m);
            return ERROR_BADPARM;
        }
    };
    for p in points.iter().take(count.max(0) as usize) {
        let line = format!(
            "{} {} {}\n",
            fmt_f(p.x, 0, 6),
            fmt_f(p.y, 0, 6),
            fmt_f(p.z, 0, 6)
        );
        if !fp.put_bytes(line.as_bytes()) {
            let m = format!("MRIwriteControlPoints({fname}): could not write file");
            log::error_printf(g, err, ERROR_BADPARM, &m);
            return ERROR_BADPARM;
        }
    }
    let user = system::user_name(env);
    let date = timer::current_date_time(env, true);
    let tail = format!(
        "info\nnumpoints {count}\nuseRealRAS {use_real_ras}\nwritten by {user} on {date}\n"
    );
    fp.put_bytes(tail.as_bytes());
    fp.close();
    NO_ERROR
}

fn print_g(x: f64) -> String {
    format_double("%g", x).expect("a %g format")
}

/// `MRIfillPoints(filled, fillval, npoints, useRealRAS, mp)`: prints `MRIfillPoints(): npoints
/// <n> useReal <u> fillval <v>  d 0.1`; the fill value is `fillval`, or for 0 (`Autodetecting
/// value to use as fill`, `  xcentroid <mean x>`) 255 when the points' mean x is below 0 and 127
/// when it is 0 or above (0 when it is NaN); `fillvaluse <v>`. For every pair of points n < m, the
/// ten points `p_n + k 0.1 (p_m - p_n)`, k = 0 to 9, are mapped by the inverse of the volume's
/// vox2ras (scanner for `useRealRAS`, else tkregister's; in `float`, FreeSurfer's matrices) and
/// rounded (`nint`); each such voxel inside the volume whose frame-0 value is not the fill value
/// is set to it. `nhits = <voxels set>`. 0. A vox2ras that cannot be inverted ends the program
/// in the product.
#[allow(clippy::too_many_arguments)]
pub fn mri_fill_points(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    filled: &mut Mri,
    fillval: i32,
    npoints: i32,
    use_real_ras: i32,
    mp: &[MPoint],
) -> Result<i32, Fatal> {
    let d = 0.1f64;
    let _ = writeln!(
        out,
        "MRIfillPoints(): npoints {npoints} useReal {use_real_ras} fillval {fillval}  d {}",
        print_g(d)
    );
    let vox2ras = if use_real_ras != 0 {
        xfm_crs2xyz(g, out, err, &filled.vg, 0, false)?
    } else {
        xfm_crs2xyz_tkreg(g, out, err, &filled.vg)?
    };
    let ras2vox = matrix_inverse(g, out, err, Some(&vox2ras), None)?.into_new();
    let n = npoints.max(0) as usize;
    let mut fillvaluse = fillval;
    if fillval == 0 {
        let _ = writeln!(out, "Autodetecting value to use as fill");
        let mut xsum = 0.0f64;
        for p in &mp[..n] {
            xsum = add64(xsum, p.x);
        }
        xsum = div64(xsum, f64::from(npoints));
        if xsum < 0.0 {
            fillvaluse = 255;
        }
        if xsum >= 0.0 {
            fillvaluse = 127;
        }
        let _ = writeln!(out, "  xcentroid {}", print_g(xsum));
    }
    let _ = writeln!(out, "fillvaluse {fillvaluse}");
    let mut ras = matrix_alloc(g, out, err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 matrix");
    *ras.relt_mut(4, 1) = 1.0;
    let mut crs: Option<larmorx_freesurfer_core::matrix::Matrix> = None;
    let mut nhits = 0i32;
    let (w, h, dz) = (filled.width, filled.height, filled.depth);
    for a in 0..n.saturating_sub(1) {
        for b in a + 1..n {
            let (pa, pb) = (mp[a], mp[b]);
            for k in 0..10 {
                let kd = mul64(f64::from(k), d);
                let x = add64(pa.x, mul64(kd, sub64(pb.x, pa.x)));
                let y = add64(pa.y, mul64(kd, sub64(pb.y, pa.y)));
                let z = add64(pa.z, mul64(kd, sub64(pb.z, pa.z)));
                *ras.relt_mut(1, 1) = x as f32;
                *ras.relt_mut(2, 1) = y as f32;
                *ras.relt_mut(3, 1) = z as f32;
                match crs.as_mut() {
                    Some(c) => {
                        matrix_multiply(g, out, err, ras2vox.as_ref(), Some(&ras), Some(c))?;
                    }
                    None => {
                        crs =
                            match matrix_multiply(g, out, err, ras2vox.as_ref(), Some(&ras), None)?
                            {
                                Ret::New(m) => Some(m),
                                _ => None,
                            };
                    }
                }
                let Some(c) = crs.as_ref() else {
                    continue;
                };
                let (cf, rf, sf) = (
                    f64::from(c.relt(1, 1)),
                    f64::from(c.relt(2, 1)),
                    f64::from(c.relt(3, 1)),
                );
                let c = utils::nint(g, cf);
                let r = utils::nint(g, rf);
                let s = utils::nint(g, sf);
                if c < 0 || i64::from(c) > i64::from(w) - 1 {
                    continue;
                }
                if r < 0 || i64::from(r) > i64::from(h) - 1 {
                    continue;
                }
                if s < 0 || i64::from(s) > i64::from(dz) - 1 {
                    continue;
                }
                let v = mri_get_vox_val(g, err, filled, c, r, s, 0, 0)?;
                if f64::from(v) == f64::from(fillvaluse) {
                    continue;
                }
                mri_set_vox_val(g, err, filled, c, r, s, 0, fillvaluse as f32, 0)?;
                nhits += 1;
            }
        }
    }
    let _ = writeln!(out, "nhits = {nhits}");
    Ok(0)
}
