// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Translations and rotations of a volume by trilinear sampling (`utils/mri.cpp`, 3.1-35):
//! `MRItranslate`, `MRIrotate_I`, `MRIrotateX_I`, `MRIrotateY_I`, `MRIrotateZ_I`.
//!
//! Defects (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-volume.md` §6):
//! - **FS-301** (rule A): both write each sample as a byte (`(unsigned char) nint(v)`) at the
//!   voxel's byte offset of the destination's row, whatever the destination's type: a volume of
//!   another type gets bytes inside its voxels. The default stores the sample with the
//!   destination's own conversion (`MRIsetVoxVal`); `reproduce` writes FreeSurfer's bytes.
//! - **FS-303** (memory): on a volume the trilinear sampler refuses (its type), each sample is
//!   the value of a variable never set (the first) or the sample before: both modes take 0 for
//!   the first, noted.
//! - **FS-304** (crash): `MRIrotateX_I`, `MRIrotateY_I` and `MRIrotateZ_I` set the result's
//!   `ras_good_flag` without checking it: an angle whose rotation `MatrixInverse` refuses (NaN,
//!   infinite) crashes after `MRIrotate_I`'s error. An error here in both modes.
//! - **Kept (rule C)**: `MRIrotateX_I`, `MRIrotateY_I` and `MRIrotateZ_I` clear a destination
//!   they are given and return a new volume (every caller passes none).

use std::io::Write;

use larmorx_core::defects::{FS_301, FS_303, FS_304};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc, matrix_multiply};
use larmorx_freesurfer_core::matrix_ops::{
    X_ROTATION, Y_ROTATION, Z_ROTATION, matrix_add, matrix_alloc_rotation, matrix_inverse,
    matrix_print,
};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_INT, MRI_LONG, MRI_SHORT, MRI_UCHAR, MRI_USHRT, Mri,
};
use larmorx_freesurfer_mri::voxel::mri_set_vox_val;

use crate::arith::{Pair, mri_clear, mri_clone};
use crate::rows::{self, Num};
use crate::sample::sample_volume;

/// The destination and where the source is: its own volume or the destination itself.
struct Dst<'a> {
    src: Option<&'a Mri>,
    dst: DstVol<'a>,
}

enum DstVol<'a> {
    New(Mri),
    Given(&'a mut Mri),
}

impl Dst<'_> {
    fn d(&mut self) -> &mut Mri {
        match &mut self.dst {
            DstVol::New(m) => m,
            DstVol::Given(m) => m,
        }
    }
    fn s(&self) -> &Mri {
        match (self.src, &self.dst) {
            (Some(s), _) => s,
            (None, DstVol::New(m)) => m,
            (None, DstVol::Given(m)) => m,
        }
    }
    fn done(self) -> Ret<Mri> {
        match self.dst {
            DstVol::New(m) => Ret::New(m),
            DstVol::Given(_) => Ret::Given,
        }
    }
}

/// `MRIclone(src, NULL)` for no destination, else `MRIclear(dst)`.
fn prepare<'a>(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair<'a>,
) -> Result<Dst<'a>, Fatal> {
    Ok(match p {
        Pair::New(s) => {
            let Ret::New(m) = mri_clone(g, out, err, s, None)? else {
                unreachable!("MRIclone without a destination returns a new volume")
            };
            Dst {
                src: Some(s),
                dst: DstVol::New(m),
            }
        }
        Pair::Given(s, d) => {
            mri_clear(g, err, d)?;
            Dst {
                src: Some(s),
                dst: DstVol::Given(d),
            }
        }
        Pair::Same(d) => {
            mri_clear(g, err, d)?;
            Dst {
                src: None,
                dst: DstVol::Given(d),
            }
        }
    })
}

/// The sample at (x1, x2, x3) and its store into voxel (y1, y2, y3) of frame 0: `(unsigned char)
/// nint(val)` at byte `y1` of the row (FS-301 for another destination type), `val` the last
/// sample when the sampler refuses the type (FS-303).
#[allow(clippy::too_many_arguments)]
fn sample_store(
    g: &mut Globals,
    err: &mut dyn Write,
    what: &str,
    v: &mut Dst,
    val: &mut f64,
    x: [f64; 3],
    y: [i32; 3],
) -> Result<(), Fatal> {
    let s = v.s();
    if !matches!(
        s.mtype,
        MRI_UCHAR | MRI_FLOAT | MRI_SHORT | MRI_USHRT | MRI_INT | MRI_LONG
    ) && !(whole(x[0]) && whole(x[1]) && whole(x[2]))
    {
        g.notices.add(
            &FS_303,
            format!(
                "{what} stores a sample of a volume type the sampler refuses: a value never set"
            ),
        );
    }
    sample_volume(g, err, s, x[0], x[1], x[2], val);
    let d = v.d();
    if d.mtype != MRI_UCHAR {
        g.notices.add(
            &FS_301,
            format!(
                "{what} writes its samples as bytes into a volume of type {}",
                d.mtype
            ),
        );
        if !g.notices.reproduce() {
            mri_set_vox_val(g, err, d, y[0], y[1], y[2], 0, *val as f32, 0)?;
            return Ok(());
        }
    }
    let b = nint(g, *val) as u8;
    rows::write(
        g,
        what,
        d,
        i64::from(y[2]),
        i64::from(y[1]),
        i64::from(y[0]),
        MRI_UCHAR,
        Num::I(i64::from(b)),
    );
    Ok(())
}

/// The samplers' `FEQUAL((int)x, x)` (whole coordinates take the nearest voxel, which sets the
/// value whatever the type).
fn whole(x: f64) -> bool {
    let i = if x.is_nan() || x >= 2_147_483_648.0 || x <= -2_147_483_649.0 {
        i32::MIN
    } else {
        x.trunc() as i32
    };
    (f64::from(i) - x).abs() < f64::from(f32::EPSILON)
}

/// `MRItranslate(mri_src, mri_dst, dx, dy, dz)`: without a destination a clone of the source
/// (`MRIclone`), else the destination cleared (`MRIclear`); each voxel `(y1, y2, y3)` of the
/// source's extent whose point `(y1 - dx, y2 - dy, y3 - dz)` lies in `[0, extent)` on every axis
/// gets [`sample_volume`]'s sample there, stored as `(unsigned char) nint(val)` (FS-301, FS-303);
/// the others stay 0. The destination's `ras_good_flag` set to 0.
#[allow(clippy::too_many_arguments)]
pub fn mri_translate(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    dx: f64,
    dy: f64,
    dz: f64,
) -> Result<Ret<Mri>, Fatal> {
    let (w, h, d) = match &p {
        Pair::New(s) | Pair::Given(s, _) => (s.vg.width, s.vg.height, s.vg.depth),
        Pair::Same(s) => (s.vg.width, s.vg.height, s.vg.depth),
    };
    let mut v = prepare(g, out, err, p)?;
    // `double val;` without a value: 0 here (FS-303)
    let mut val = 0.0f64;
    for y3 in 0..d {
        let x3 = f64::from(y3) - dz;
        if x3 < 0.0 || x3 >= f64::from(d) {
            continue;
        }
        for y2 in 0..h {
            let x2 = f64::from(y2) - dy;
            if x2 < 0.0 || x2 >= f64::from(h) {
                continue;
            }
            // `pdst = &MRIvox(mri_dst, 0, y2, y3)`: the row pointer read
            rows::touch(g, "MRItranslate", v.d(), i64::from(y3), i64::from(y2));
            for y1 in 0..w {
                let x1 = f64::from(y1) - dx;
                if x1 >= 0.0 && x1 < f64::from(w) {
                    sample_store(
                        g,
                        err,
                        "MRItranslate",
                        &mut v,
                        &mut val,
                        [x1, x2, x3],
                        [y1, y2, y3],
                    )?;
                }
            }
        }
    }
    v.d().vg.ras_good_flag = 0;
    Ok(v.done())
}

/// `MRIrotate_I(mri_src, mri_dst, mR, mO)`: the inverse of `mR` (`MatrixInverse`; `NULL` gives
/// the error `MRIrotate_I: rotation matrix is singular` and `NULL`); without a destination a clone
/// of the source, else the destination cleared; the centre `mO` (3 x 1) or, without one, the
/// source's half extents (`width / 2.0`, as `float`), truncated to `int` (`y1o`, `y2o`, `y3o`).
/// For each voxel (z outermost) the point `mRinv (y - yo) + mO` (`MatrixMultiply`, then
/// `MatrixAdd`, in `float`) is sampled ([`sample_volume`]) and stored as `(unsigned char)
/// nint(val)` (FS-301, FS-303); the destination's `ras_good_flag` set to 0. With `Gdiag` 99 the
/// inverse and the centre are printed (`MatrixPrint`) on standard output.
pub fn mri_rotate_i(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    p: Pair,
    m_r: &Matrix,
    m_o: Option<&Matrix>,
) -> Result<Ret<Mri>, Fatal> {
    let Ret::New(rinv) = matrix_inverse(g, out, err, Some(m_r), None)? else {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            "MRIrotate_I: rotation matrix is singular",
        );
        return Ok(Ret::Null);
    };
    let (w, h, d) = match &p {
        Pair::New(s) | Pair::Given(s, _) => (s.vg.width, s.vg.height, s.vg.depth),
        Pair::Same(s) => (s.vg.width, s.vg.height, s.vg.depth),
    };
    let mut v = prepare(g, out, err, p)?;
    let own_o;
    let m_o = match m_o {
        Some(m) => m,
        None => {
            let mut m = matrix_alloc(g, out, err, 3, 1, MATRIX_REAL)?.expect("3 x 1");
            m.set_relt(1, 1, (f64::from(w) / 2.0) as f32);
            m.set_relt(2, 1, (f64::from(h) / 2.0) as f32);
            m.set_relt(3, 1, (f64::from(d) / 2.0) as f32);
            own_o = m;
            &own_o
        }
    };
    let mut mx = matrix_alloc(g, out, err, 3, 1, MATRIX_REAL)?.expect("3 x 1");
    let mut my = matrix_alloc(g, out, err, 3, 1, MATRIX_REAL)?.expect("3 x 1");
    let cvt = |f: f32| -> i32 {
        let x = f64::from(f);
        if x.is_nan() || x >= 2_147_483_648.0 || x <= -2_147_483_649.0 {
            i32::MIN
        } else {
            x.trunc() as i32
        }
    };
    let y1o = cvt(m_o.relt(1, 1));
    let y2o = cvt(m_o.relt(2, 1));
    let y3o = cvt(m_o.relt(3, 1));
    if g.gdiag == 99 {
        matrix_print(
            g,
            Some(&mut *out),
            &mut std::io::sink(),
            err,
            Some(&rinv),
            5,
        );
        matrix_print(g, Some(&mut *out), &mut std::io::sink(), err, Some(m_o), 5);
    }
    let mut val = 0.0f64;
    for y3 in 0..d {
        my.set_relt(3, 1, y3.wrapping_sub(y3o) as f32);
        for y2 in 0..h {
            my.set_relt(2, 1, y2.wrapping_sub(y2o) as f32);
            for y1 in 0..w {
                my.set_relt(1, 1, y1.wrapping_sub(y1o) as f32);
                matrix_multiply(g, out, err, Some(&rinv), Some(&my), Some(&mut mx))?;
                let prod = mx.clone();
                matrix_add(g, out, err, &prod, m_o, Some(&mut mx))?;
                let x = [
                    f64::from(mx.relt(1, 1)),
                    f64::from(mx.relt(2, 1)),
                    f64::from(mx.relt(3, 1)),
                ];
                sample_store(g, err, "MRIrotate_I", &mut v, &mut val, x, [y1, y2, y3])?;
            }
        }
    }
    v.d().vg.ras_good_flag = 0;
    Ok(v.done())
}

/// `MRIrotateX_I(mri_src, mri_dst, x_angle)` (and `Y`, `Z` with [`mri_rotate_y_i`],
/// [`mri_rotate_z_i`]): a given destination is cleared (`MRIclear`); the result is always a new
/// volume, [`mri_rotate_i`] of the source with `MatrixAllocRotation(3, angle, X_ROTATION)` about
/// its centre, or `None` where that is `NULL` (FreeSurfer then crashes setting its
/// `ras_good_flag`: an error here).
pub fn mri_rotate_x_i(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    angle: f32,
) -> Result<Option<Mri>, Fatal> {
    rotate_axis(g, out, err, src, dst, angle, X_ROTATION, "MRIrotateX_I")
}

/// `MRIrotateY_I(mri_src, mri_dst, y_angle)`: see [`mri_rotate_x_i`].
pub fn mri_rotate_y_i(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    angle: f32,
) -> Result<Option<Mri>, Fatal> {
    rotate_axis(g, out, err, src, dst, angle, Y_ROTATION, "MRIrotateY_I")
}

/// `MRIrotateZ_I(mri_src, mri_dst, z_angle)`: see [`mri_rotate_x_i`].
pub fn mri_rotate_z_i(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    angle: f32,
) -> Result<Option<Mri>, Fatal> {
    rotate_axis(g, out, err, src, dst, angle, Z_ROTATION, "MRIrotateZ_I")
}

#[allow(clippy::too_many_arguments)]
fn rotate_axis(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    angle: f32,
    which: i32,
    what: &str,
) -> Result<Option<Mri>, Fatal> {
    // FreeSurfer clones the source here when there is no destination (a volume it then loses)
    if let Some(d) = dst {
        mri_clear(g, err, d)?;
    }
    let Ret::New(m) = matrix_alloc_rotation(g, out, err, 3, angle, which)? else {
        unreachable!("MatrixAllocRotation returns a new matrix")
    };
    match mri_rotate_i(g, out, err, Pair::New(src), &m, None)? {
        Ret::New(mut r) => {
            r.vg.ras_good_flag = 0;
            Ok(Some(r))
        }
        _ => {
            g.notices
                .add(&FS_304, format!("{what} after MRIrotate_I returned NULL"));
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                &format!("{what}(): no rotated volume (FreeSurfer crashes here)\n"),
            )?;
            Ok(None)
        }
    }
}
