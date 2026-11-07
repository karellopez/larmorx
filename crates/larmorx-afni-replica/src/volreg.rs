// SPDX-License-Identifier: Apache-2.0 AND CC-BY-4.0
// Translated to Rust from AFNI 26.3.05: src/3dvolreg.c (the work of main after the option scan:
// the -maxdisp mask, the -twopass first pass with voldif, get_best_shift, new_voldif,
// new_get_best_shift and new_get_best_shiftrot, the final pass, -rotparent's final
// transformation, the parameters, the statistics, the matrices, the displacements and
// VL_get_displacments), src/thd_rotangles.c (THD_rotcom_to_matvec,
// THD_rotangle_user_to_dset, mangle_angle, THD_axcode, THD_handedness, DBLE_mat_to_dicomm),
// src/thd_center.c (THD_dataset_center), src/thd_coords.c (THD_3dind_to_3dmm_no_wod,
// THD_3dmm_to_dicomm), src/mri_to_short.c (mri_to_short with scale 1), src/mri_to_byte.c
// (mri_to_byte) and src/mri_allzero.c (mri_nonzero_count). Major portions Copyright (C)
// 1994-2000 Medical College of Wisconsin (AFNI, Robert W. Cox et al.), released by AFNI 26.3.05
// under the Creative Commons Attribution 4.0 International licence (CC BY 4.0). thd_center.c and
// mri_allzero.c carry no such notice: they were written at the NIH and are in the public domain.
// CC BY 4.0: https://creativecommons.org/licenses/by/4.0/ (the material is provided as is, without
// warranties of any kind); source: https://github.com/afni/afni, tag AFNI_26.3.05.
// AFNI asks that work based on it cite its authors (LICENSE.txt).
// Rust translation Copyright 2026 Karel Lopez Vilaret, licensed under the Apache License 2.0;
// modified from the original: translated to Rust, with the changes PROVENANCE.md lists.
//! 3dvolreg's registration of every sub-brick of a dataset to a base brick, bit for bit.
//!
//! [`volreg`] runs what AFNI's `main()` runs after its option scan: the `-maxdisp` mask (the
//! boundary of `THD_automask` with clip fraction 0.333), the optional first pass of `-twopass`
//! (blurred base and bricks, linear interpolation, a coarse search over shifts and rotations
//! for the starting point, and a first-pass weight from the blurred sum of the base and the
//! first brick), the final pass, the parameters in AFNI's conventions (degrees about the I, R
//! and A axes, mm along S, L and P), the registered bricks in the input's storage type, the
//! maximum displacements and the matrices.
//!
//! **Upstream defects (D13).** By default larmorx corrects AFNI's defects on this path and
//! names each one that changed a result; [`UpstreamDefects::Reproduce`] gives AFNI's result:
//! - AFNI-87: a byte dataset's registered bricks are stretched to 0..255.9 each (mri_to_byte);
//!   by default they are rounded and clamped as bytes;
//! - AFNI-92: `-maxdisp` (the default) sets the automask's clip fraction to 0.333 for the rest
//!   of the run, so `-nomaxdisp` changes `-coarserot`'s mask (0.5); by default 0.333 always;
//! - AFNI-93: a coarse search whose overlap region is empty scores every candidate 0 and keeps
//!   the first; by default such candidates are not taken;
//! - AFNI-94: a coarse step of 1 voxel on a volume of 120 or more voxels along every axis
//!   halves to 0 for the subsampled search, an endless loop in AFNI: an error in both modes;
//! - AFNI-89: a least squares AFNI cannot set up (all-zero weights, an all-zero weighted
//!   derivative) crashes AFNI at the first registered brick: an error in both modes, after
//!   AFNI's messages.
//! - AFNI-97: a Fourier rotation whose shear shifts the last of an odd count of rows and the
//!   previous pair's second row beyond the row length (rotations far beyond a head's motion,
//!   random testing's trial of 2026-11-06): `fft_shift2` clears the missing second row through
//!   its NULL pointer and AFNI crashes; by default the row that exists is cleared alone (as
//!   AFNI clears a pair), with `reproduce` an error where AFNI crashes, after its messages.

use larmorx_core::defects::{self, Notices, UpstreamDefects};

use crate::align3d::{self, Align3d, Basis, Vol, mri_max};
use crate::automask;
use crate::axes::{ORI_A2P, ORI_I2S, ORI_L2R, ORI_P2A, ORI_R2L, ORI_S2I};
use crate::blur::Blur;
use crate::dataset::{BrickData, Dataset};
use crate::mask::{self, Grid, MaskSettings};
use crate::message::Message;
use crate::rot3d::Rotator;
use crate::shift::Method;
use crate::vecmat::{self, DMat33, DVec3, DVecMat, FVec3};

/// `PI` of `mrilib.h`.
const PI: f64 = std::f64::consts::PI;

/// The interpolation names of 3dvolreg's `modes[]`, indexed by AFNI's `MRI_NN` (0) to
/// `MRI_HEPTIC` (5).
pub fn mode_name(m: Method) -> &'static str {
    match m {
        Method::Nn => "-NN",
        Method::Linear => "-linear",
        Method::Cubic => "-cubic",
        Method::Fourier => "-Fourier",
        Method::Quintic => "-quintic",
        Method::Heptic => "-heptic",
        _ => "-?",
    }
}

/// The base brick: a sub-brick of the input, or a volume from another dataset (its sub-brick 0,
/// scaled to float), with that dataset's centre and origin.
#[derive(Clone, Debug)]
pub enum Base {
    Index(usize),
    External {
        vol: Vec<f32>,
        /// `THD_dataset_center` of the base dataset.
        center: FVec3,
        /// `xxorg, yyorg, zzorg` of the base dataset (for `-twodup`).
        origin: [f32; 3],
    },
}

/// `-edging`: a voxel count (`ee`) or a percentage of each dimension (`ee%`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Edging {
    Voxels(f32),
    Percent(f32),
}

/// What the command line decided (3dvolreg's `VL_*` globals).
#[derive(Clone, Debug)]
pub struct VolregParams {
    pub base: Base,
    /// `-weight`'s brick (scaled to float), when given.
    pub weight: Option<Vec<f32>>,
    pub wtinp: bool,
    pub resam: Method,
    pub final_mode: Option<Method>,
    pub clipit: bool,
    pub zpad: usize,
    pub twopass: bool,
    pub twoblur: f32,
    pub sinit: bool,
    pub coarse_del: i32,
    pub coarse_num: i32,
    pub coarse_rot: bool,
    pub edging: Edging,
    pub maxite: i32,
    pub dxy: f32,
    pub dph: f32,
    pub del: f32,
    pub maxdisp: bool,
    pub verbose: u32,
    pub floatize: bool,
    pub firfac: f64,
    pub firold: bool,
    /// `-rotparent`: the final transformation applied on top of each sub-brick's own, instead
    /// of the registration's own resampling.
    pub matvec: Option<Matvec>,
    /// For measurement only: the blur's rows mirrored about their end points on every axis
    /// ([`Blur::whole_point_mirrors`]); `false` (AFNI's mirrors) in both D13 modes.
    pub whole_point_mirrors: bool,
    pub mode: UpstreamDefects,
}

/// `-rotparent`'s transformation (`rmat`, `tvec` of 3dvolreg's main, in the dataset's axis
/// order) and `-gridparent`'s zero padding along the third axis (`npad_neg` slices before the
/// first, `npad_pos` after the last).
#[derive(Clone, Debug, PartialEq)]
pub struct Matvec {
    pub rmat: DMat33,
    pub tvec: DVec3,
    pub npad: [usize; 2],
}

/// The results, per sub-brick and overall.
#[derive(Clone, Debug, Default)]
pub struct VolregResult {
    pub roll: Vec<f32>,
    pub pitch: Vec<f32>,
    pub yaw: Vec<f32>,
    pub dx: Vec<f32>,
    pub dy: Vec<f32>,
    pub dz: Vec<f32>,
    pub rmsold: Vec<f32>,
    pub rmsnew: Vec<f32>,
    /// The registered dataset (the input's geometry, the output's storage type).
    pub output: Option<Dataset>,
    /// `-maxdisp`: the maximum displacement and the maximum change of displacement of each
    /// sub-brick (mm), and the largest of each with its sub-brick.
    pub maxdisp: Option<MaxDisp>,
    /// The `VOLREG_MATVEC_*` matrices (`a11 a12 a13 v1 ... v3`, base to input in DICOM order
    /// inverted: input to base) of each sub-brick, as floats.
    pub matvec: Vec<[f32; 12]>,
    /// The rotation of each `VOLREG_MATVEC` in double (DICOM order), as `-savedisp` uses it.
    pub rmat: Vec<DMat33>,
    /// The `-1Dmatrix_save` rows: the inverse of each `VOLREG_MATVEC`, about the centres.
    pub matrix_save: Vec<[f64; 12]>,
}

/// `-maxdisp`'s results.
#[derive(Clone, Debug, Default)]
pub struct MaxDisp {
    pub dmax: Vec<f32>,
    pub emax: Vec<f32>,
    pub top: (f32, usize),
    pub top_delta: (f32, usize),
    /// AFNI-99: the same values in the other mode (AFNI's mirrored positions with the default,
    /// the voxels' own with `reproduce`), computed when an axis is stored L2R, P2A or S2I, so
    /// that the command line names the defect exactly where a printed value differs.
    pub other: Option<Box<MaxDisp>>,
}

/// Why the registration stopped.
#[derive(Debug, Clone, PartialEq)]
pub enum VolregError {
    /// `ERROR_exit` (`** FATAL ERROR: `).
    Fatal(String),
    /// A `fprintf(stderr, "** ...")` and `exit(1)`: the text as AFNI prints it.
    Plain(String),
    /// Where AFNI crashes (AFNI-89, AFNI-97): larmorx's error, on a line of its own (AFNI's
    /// crash banner starts a new line after a progress line such as "0..1").
    Crash(String),
}

/// The dataset's orientation codes, or an error for a degenerate matrix (AFNI indexes its
/// tables out of bounds).
fn orient(ds: &Dataset) -> Result<[u8; 3], VolregError> {
    ds.geometry.axes.orient.ok_or_else(|| {
        VolregError::Fatal(
            "the dataset's voxel-to-world matrix has no orientation (a zero or repeated \
             column): not supported by larmorx"
                .to_owned(),
        )
    })
}

/// `THD_axcode(dset, ori)`: `+k` if `ori` (I S R L A P) is the positive direction of axis `k`
/// (1, 2, 3), `-k` if the negative one, -99 if neither.
pub fn axcode(o: [u8; 3], ori: char) -> i32 {
    let ori = ori.to_ascii_uppercase();
    for (k, &code) in o.iter().enumerate() {
        let t = crate::automask::tinystr(Some(code));
        if ori == t[0] {
            return k as i32 + 1;
        }
        if ori == t[1] {
            return -(k as i32 + 1);
        }
    }
    -99
}

/// `THD_handedness`: +1 if the dataset's axes are right-handed, -1 if not.
pub fn handedness(o: [u8; 3]) -> i32 {
    let mut q = [[0.0f32; 3]; 3];
    for (col, &code) in o.iter().enumerate() {
        match code {
            0 => q[0][col] = 1.0,
            1 => q[0][col] = -1.0,
            2 => q[1][col] = -1.0,
            3 => q[1][col] = 1.0,
            4 => q[2][col] = 1.0,
            _ => q[2][col] = -1.0,
        }
    }
    let val =
        q[0][0] * q[1][1] * q[2][2] - q[0][0] * q[1][2] * q[2][1] - q[1][0] * q[0][1] * q[2][2]
            + q[1][0] * q[0][2] * q[2][1]
            + q[2][0] * q[0][1] * q[1][2]
            - q[2][0] * q[0][2] * q[1][1];
    if val > 0.0 { 1 } else { -1 }
}

/// `DBLE_mat_to_dicomm`: the permutation-like matrix from the dataset's axes to DICOM order.
pub fn mat_to_dicomm(o: [u8; 3]) -> DMat33 {
    let mut t = [[0.0; 3]; 3];
    for (col, &code) in o.iter().enumerate() {
        match code {
            ORI_R2L => t[0][col] = 1.0,
            ORI_L2R => t[0][col] = -1.0,
            ORI_P2A => t[1][col] = -1.0,
            ORI_A2P => t[1][col] = 1.0,
            ORI_I2S => t[2][col] = 1.0,
            _ => t[2][col] = -1.0,
        }
    }
    t
}

/// [`mat_to_dicomm`] without its signs: the permutation of the axes. AFNI-99 corrected: the
/// voxel positions of `-maxdisp` and `-savedisp` are AFNI's dataset coordinates, DICOM values
/// along each axis, which this permutation (not the signed matrix) converts.
pub fn mat_to_dicomm_unsigned(o: [u8; 3]) -> DMat33 {
    mat_to_dicomm(o).map(|row| row.map(f64::abs))
}

/// The conversion matrix of `-maxdisp` and `-savedisp`: AFNI's signed one with `reproduce`
/// (AFNI-99), the permutation by default.
pub fn disp_conversion(o: [u8; 3], mode: UpstreamDefects) -> DMat33 {
    if mode.reproduce() {
        mat_to_dicomm(o)
    } else {
        mat_to_dicomm_unsigned(o)
    }
}

/// Whether AFNI-99 can change anything: an axis stored L2R, P2A or S2I.
pub fn has_negative_axis(o: [u8; 3]) -> bool {
    mat_to_dicomm(o).iter().flatten().any(|&v| v < 0.0)
}

/// `THD_3dmm_to_dicomm`: dataset-order coordinates to DICOM order.
fn mm_to_dicomm(o: [u8; 3], v: FVec3) -> FVec3 {
    let mut d = [0.0f32; 3];
    for (axis, &code) in o.iter().enumerate() {
        d[crate::axes::coordinate(code)] = v[axis];
    }
    d
}

/// `THD_dataset_center`: the DICOM coordinates of the centre of the grid (in float).
pub fn dataset_center(ds: &Dataset) -> Result<FVec3, VolregError> {
    let o = orient(ds)?;
    let ax = &ds.geometry.axes;
    let n = ds.nxyz;
    let fv1 = mm_to_dicomm(o, ax.org);
    let fv2 = mm_to_dicomm(
        o,
        [
            ax.org[0] + (n[0] as f32 - 1.0) * ax.del[0],
            ax.org[1] + (n[1] as f32 - 1.0) * ax.del[1],
            ax.org[2] + (n[2] as f32 - 1.0) * ax.del[2],
        ],
    );
    Ok([
        0.5f32 * (fv1[0] + fv2[0]),
        0.5f32 * (fv1[1] + fv2[1]),
        0.5f32 * (fv1[2] + fv2[2]),
    ])
}

/// `"%.4f"` of a float, read back by `sscanf("%f")`: the angles `THD_rotcom_to_matvec` sees.
fn rotcom_value(x: f32) -> f32 {
    format!("{x:.4}").parse::<f32>().unwrap_or(0.0)
}

/// `THD_rotcom_to_matvec(dset, "-rotate <r>I <p>R <y>A")` (no shift): the rotation, each angle
/// first printed with `%.4f` and read back.
pub fn rotcom_rotation(o: [u8; 3], r: f32, p: f32, y: f32, libc: vecmat::Libc) -> DMat33 {
    let mangle = |th: f32, ax: char| -> (f32, i32) {
        let code = axcode(o, ax);
        let neg = code < 0;
        let ax1 = code.abs() - 1;
        (if neg { -th } else { th }, ax1)
    };
    let (mut d1, a1) = mangle(rotcom_value(r), 'I');
    let (mut d2, a2) = mangle(rotcom_value(p), 'R');
    let (mut d3, a3) = mangle(rotcom_value(y), 'A');
    if handedness(o) < 0 {
        d1 = -d1;
        d2 = -d2;
        d3 = -d3;
    }
    vecmat::rot_to_matrix(
        a1,
        -(PI / 180.0) * f64::from(d1),
        a2,
        -(PI / 180.0) * f64::from(d2),
        a3,
        -(PI / 180.0) * f64::from(d3),
        libc,
    )
}

/// `THD_rotcom_to_matvec(dset, "-rotate <r>I <p>R <y>A -ashift <s>S <l>L <q>P")`: the rotation
/// of [`rotcom_rotation`] and the shift, each value printed with `%.4f` and read back; a shift
/// along an axis's positive direction is negated (`THD_axcode`).
pub fn rotcom_matvec(
    o: [u8; 3],
    angles: [f32; 3],
    shifts: [f32; 3],
    libc: vecmat::Libc,
) -> DVecMat {
    let mm = rotcom_rotation(o, angles[0], angles[1], angles[2], libc);
    let mut q = [0.0f32; 3];
    for (k, (&v, ch)) in shifts.iter().zip(['S', 'L', 'P']).enumerate() {
        let v = rotcom_value(v);
        let code = axcode(o, ch);
        // The `default:` of each switch: the axis in the shift's own position, negative.
        let code = if (1..=3).contains(&code.abs()) {
            code
        } else {
            -(k as i32 + 1)
        };
        let axis = code.unsigned_abs() as usize - 1;
        q[axis] = if code > 0 { -v } else { v };
    }
    DVecMat {
        mm,
        vv: q.map(f64::from),
    }
}

/// `VL_get_displacments(dset, rmat, tvec)`: each voxel's displacement (mm, the dataset's axis
/// order) by the transformation `rmat`, `tvec` (DICOM order) about the centre of `out` (the
/// output's grid, `DSET_XCEN`), at the coordinates of the input's grid (`input`, whose axes
/// give each voxel's position).
#[allow(clippy::needless_range_loop)] // AFNI's voxel loop, as written
pub fn displacements(
    input: &crate::axes::Axes,
    out: &crate::axes::Axes,
    out_n: [usize; 3],
    rmat: &DMat33,
    tvec: &DVec3,
    mode: UpstreamDefects,
) -> Result<[Vec<f32>; 3], VolregError> {
    let o = out.orient.ok_or_else(|| {
        VolregError::Fatal("the dataset's voxel-to-world matrix has no orientation".to_owned())
    })?;
    let [nx, ny, nz] = out_n;
    let nxy = nx * ny;
    let nxyz = nxy * nz;
    let centre = |a: usize| {
        (f64::from(out.org[a]) + 0.5 * (out_n[a] as f64 - 1.0) * f64::from(out.del[a])) as f32
    };
    let vorg: DVec3 = [
        f64::from(centre(0)),
        f64::from(centre(1)),
        f64::from(centre(2)),
    ];
    let pp = disp_conversion(o, mode);
    let ppt = vecmat::transpose(&pp);
    let r = vecmat::mul(&ppt, rmat);
    let r = vecmat::mul(&r, &pp);
    let t = vecmat::matvec(&ppt, tvec);
    let mut disp = [vec![0.0f32; nxyz], vec![0.0f32; nxyz], vec![0.0f32; nxyz]];
    for mm in 0..nxyz {
        let (ii, kk, jj) = (mm % nx, mm / nxy, (mm % nxy) / nx);
        let fv = [
            input.org[0] + ii as f32 * input.del[0],
            input.org[1] + jj as f32 * input.del[1],
            input.org[2] + kk as f32 * input.del[2],
        ];
        let dv: DVec3 = fv.map(f64::from);
        let mut qv = vecmat::sub(&dv, &vorg);
        qv = vecmat::matvec_add(&r, &qv, &t);
        qv = vecmat::add(&qv, &vorg);
        qv = vecmat::sub(&dv, &qv);
        for a in 0..3 {
            disp[a][mm] = qv[a] as f32;
        }
    }
    Ok(disp)
}

/// `mri_nonzero_count`.
fn nonzero_count(v: &[f32]) -> usize {
    v.iter().filter(|&&x| x != 0.0).count()
}

/// `voldif` (`w` = None) and `new_voldif`: the residual sum of squares of `b` against the best
/// multiple of `v` shifted by `(dx, dy, dz)`, over the voxels at least `edge` from every face,
/// in float. `None` for AFNI-93's empty region (the caller decides).
#[allow(clippy::too_many_arguments)]
fn voldif(
    n: [usize; 3],
    b: &[f32],
    d: [i32; 3],
    v: &[f32],
    edge: i32,
    w: Option<&[f32]>,
) -> (f32, bool) {
    let [nx, ny, nz] = n.map(|x| x as i32);
    let nxy = nx * ny;
    let (nxtop, nytop, nztop) = (nx - edge, ny - edge, nz - edge);
    let (mut bbsum, mut vvsum, mut bvsum) = (0.0f32, 0.0f32, 0.0f32);
    let empty = nxtop <= edge || nytop <= edge || nztop <= edge;
    for kk in edge..nztop {
        for jj in edge..nytop {
            for ii in edge..nxtop {
                let bi = (ii + jj * nx + kk * nxy) as usize;
                let vi = ((ii - d[0]) + (jj - d[1]) * nx + (kk - d[2]) * nxy) as usize;
                match w {
                    None => {
                        let (bb, vv) = (b[bi], v[vi]);
                        bbsum += bb * bb;
                        vvsum += vv * vv;
                        bvsum += bb * vv;
                    }
                    Some(w) => {
                        let ww = w[bi];
                        if ww > 0.0 {
                            let (bb, vv) = (b[bi], v[vi]);
                            bbsum += ww * bb * bb;
                            vvsum += ww * vv * vv;
                            bvsum += ww * bb * vv;
                        }
                    }
                }
            }
        }
    }
    if vvsum > 0.0 {
        bbsum -= bvsum * bvsum / vvsum;
    }
    (bbsum, empty)
}

/// The state 3dvolreg's coarse search changes: `VL_coarse_del` (halved and doubled back by the
/// subsampled search).
struct Coarse {
    del: i32,
    num: i32,
}

/// `get_best_shift` (`w` = None) and `new_get_best_shift`: the shift (multiples of `del` up to
/// `del·num` along each axis, within a sphere) whose residual is smallest, starting from no
/// shift with the residual `Σ b²`; returns the residual and the shift.
fn best_shift(
    n: [usize; 3],
    b: &[f32],
    v: &[f32],
    w: Option<&[f32]>,
    c: &Coarse,
    fix_empty: bool,
    met_empty: &mut bool,
) -> (f32, [i32; 3]) {
    let shift = c.del;
    let numsh = c.num;
    let shtop = shift * numsh;
    let edge = shtop + shift;
    let sqtop = shtop * shtop;
    let mut bsum = 0.0f32;
    for &x in &b[..n[0] * n[1] * n[2]] {
        bsum += x * x;
    }
    let mut best = [0i32; 3];
    let mut dz = -shtop;
    while dz <= shtop {
        let mut dy = -shtop;
        while dy <= shtop {
            let mut dx = -shtop;
            while dx <= shtop {
                if dx * dx + dy * dy + dz * dz <= sqtop {
                    let (sum, empty) = voldif(n, b, [dx, dy, dz], v, edge, w);
                    if empty {
                        *met_empty = true;
                    }
                    if !(empty && fix_empty) && sum < bsum {
                        bsum = sum;
                        best = [dx, dy, dz];
                    }
                }
                dx += shift;
            }
            dy += shift;
        }
        dz += shift;
    }
    (bsum, best)
}

/// `DANGLE`, `NROLL`, `NPITCH`, `NYAW` of `new_get_best_shiftrot`.
const DANGLE: f32 = 9.0;

/// The volume 3dvolreg works on, with what its coarse rotation search needs.
struct Ctx<'a> {
    p: &'a VolregParams,
    o: [u8; 3],
    n: [usize; 3],
    d: [f32; 3],
    notices: &'a mut Notices,
}

/// `new_get_best_shiftrot(dset, base, vol, ...)`: the best of 3 × 5 × 3 rotations (roll, pitch,
/// yaw in steps of 9°) of `vol`, each nearest-neighbour rotated and searched for its best
/// shift against `base` with a weight (the blurred, normalised and automasked |base|).
/// Volumes of 120 or more voxels along every axis are first blurred and subsampled by 2.
/// Returns `(roll, pitch, yaw)` and the shift in voxels.
#[allow(clippy::too_many_arguments)]
fn best_shiftrot(
    ctx: &mut Ctx<'_>,
    base: &[f32],
    vol: &[f32],
    coarse: &mut Coarse,
    rot: &mut Rotator,
    blur: &Blur,
    clfrac: f32,
    log: &mut Vec<Message>,
) -> Result<([f32; 3], [i32; 3]), VolregError> {
    let verbose = ctx.p.verbose > 0;
    let mut n = ctx.n;
    let mut d = ctx.d;
    let mut bim = base.to_vec();
    let mut vim = vol.to_vec();
    let mut subd = false;
    if n[0] >= 120 && n[1] >= 120 && n[2] >= 120 {
        let [nx, ny, nz] = n;
        let h = [nx.div_ceil(2), ny.div_ceil(2), nz.div_ceil(2)];
        if verbose {
            log.push(Message::Raw("x".to_owned()));
        }
        let sub = |src: &[f32]| -> Vec<f32> {
            let mut t = src.to_vec();
            blur.fir_blur_volume(n, [1.0; 3], &mut t, 1.0);
            let mut out = vec![0.0f32; h[0] * h[1] * h[2]];
            for kk in 0..h[2] {
                for jj in 0..h[1] {
                    for ii in 0..h[0] {
                        out[ii + jj * h[0] + kk * h[0] * h[1]] =
                            t[2 * (ii + jj * nx + kk * nx * ny)];
                    }
                }
            }
            out
        };
        bim = sub(base);
        vim = sub(vol);
        d = d.map(|x| 2.0f32 * x);
        n = h;
        subd = true;
        coarse.del /= 2;
        if coarse.del == 0 {
            // AFNI-94: AFNI loops forever with a step of 0.
            ctx.notices.add(
                &defects::AFNI_94,
                "-twopass's coarse search with a step of 1 voxel on a volume of 120 or more \
                 voxels along every axis: the subsampled search's step is 0, which AFNI \
                 repeats forever",
            );
            return Err(VolregError::Fatal(
                "-coarse with a step of 1 voxel on a volume of 120 or more voxels along every \
                 axis: the subsampled coarse search would have a step of 0 (AFNI loops forever; \
                 AFNI-94); use -coarse 2 or more"
                    .to_owned(),
            ));
        }
    }
    let nxyz = n[0] * n[1] * n[2];
    if verbose {
        log.push(Message::Raw("w".to_owned()));
    }
    let mut www: Vec<f32> = bim.iter().map(|x| x.abs()).collect();
    blur.fir_blur_volume(n, [1.0; 3], &mut www, 1.0);
    let mut wtop = 0.0f32;
    for &x in &www {
        wtop = if wtop < x { x } else { wtop };
    }
    wtop = 1.0f32 / wtop;
    for x in &mut www {
        *x *= wtop;
        if f64::from(*x) < 0.05 {
            *x = 0.0;
        }
    }
    let mut ms = MaskSettings::default();
    ms.set_clipfrac(clfrac);
    let mut mlog = Vec::new();
    let mmm = mask::automask_image(&www, Grid::new(n[0], n[1], n[2]), &ms, &mut mlog)
        .unwrap_or_else(|| vec![0u8; nxyz]);
    for (w, &m) in www.iter_mut().zip(&mmm) {
        if m == 0 {
            *w = 0.0;
        }
    }
    if verbose {
        let c = mask::count(&mmm);
        log.push(Message::Raw(format!(
            "[{:.1}%]",
            (100.0 * f64::from(c)) / nxyz as f64
        )));
    }
    rot.method = Method::Nn;
    let fix = ctx.p.mode.fix();
    let mut met_empty = false;
    let mut bsum = 1.0e38f32;
    let mut best = ([0.0f32; 3], [0i32; 3]);
    for kk in -1i32..=1 {
        for jj in -2i32..=2 {
            for ii in -1i32..=1 {
                let r = kk as f32 * DANGLE;
                let p = jj as f32 * DANGLE;
                let y = ii as f32 * DANGLE;
                let tim = if r == 0.0 && p == 0.0 && y == 0.0 {
                    None
                } else {
                    let mm = rotcom_rotation(ctx.o, r, p, y, rot.libc);
                    let t = rot
                        .rota3d_matvec(&vim, n, d, &mm, &[0.0; 3])
                        .map_err(|e| VolregError::Fatal(e.to_string()))?;
                    for m in rot.messages.drain(..) {
                        log.push(Message::Raw(m));
                    }
                    Some(t)
                };
                let tar = tim.as_deref().unwrap_or(&vim);
                let mut empty = false;
                let (sum, sh) = best_shift(n, &bim, tar, Some(&www), coarse, fix, &mut empty);
                met_empty |= empty;
                // AFNI-93 corrected: a rotation whose shift search summed over no voxel (the
                // region is the same for every shift) is not a candidate either.
                let take = sum < bsum && !(fix && empty);
                if verbose {
                    log.push(Message::Raw(if take { "*" } else { "." }.to_owned()));
                }
                if take {
                    best = ([r, p, y], sh);
                    bsum = sum;
                }
            }
        }
    }
    note_empty(ctx, met_empty);
    let (ang, mut sh) = best;
    if subd {
        sh = sh.map(|x| 2 * x);
        coarse.del *= 2;
    }
    Ok((ang, sh))
}

/// AFNI-93's notice: a coarse search met candidates whose overlap region is empty.
fn note_empty(ctx: &mut Ctx<'_>, met: bool) {
    if !met {
        return;
    }
    let text = if ctx.p.mode.fix() {
        "the coarse search's region (the voxels at least (num+1)*del from every face) is empty \
         for this volume: those shifts were not taken; AFNI scores each 0 and keeps the first \
         (a large initial shift and rotation)"
    } else {
        "the coarse search's region (the voxels at least (num+1)*del from every face) is empty \
         for this volume: every candidate scores 0 and the first is kept, as AFNI does"
    };
    ctx.notices.add(&defects::AFNI_93, text);
}

/// The registered brick in the output's storage type, from the float volume (`tim`).
fn store(tim: &[f32], kind: Kind, mode: UpstreamDefects, notices: &mut Notices) -> BrickData {
    match kind {
        Kind::Float => BrickData::Float(tim.to_vec()),
        Kind::Short => BrickData::Short(tim.iter().map(|&x| crate::tshift::shortize(x)).collect()),
        Kind::Byte => {
            // mri_to_byte: stretched to 0..255.9 from the brick's own range (AFNI-87).
            let imin = align3d::mri_min(tim);
            let mut imax = align3d::mri_max(tim);
            if imax <= imin {
                imax = imin + 1.0;
            }
            let scale = 255.9 / (imax - imin);
            let flbot = imin as f32;
            let stretched: Vec<u8> = tim
                .iter()
                .map(|&x| (scale * f64::from(x - flbot)) as i64 as u8)
                .collect();
            let rounded: Vec<u8> = tim.iter().map(|&x| crate::tshift::byteize(x)).collect();
            if stretched != rounded {
                notices.add(
                    &defects::AFNI_87,
                    if mode.fix() {
                        "a byte dataset's registered sub-bricks are written as rounded bytes; \
                         AFNI stretches each to 0..255.9 from its own minimum and maximum"
                    } else {
                        "a byte dataset's registered sub-bricks are stretched to 0..255.9 from \
                         their own minimum and maximum, as AFNI does"
                    },
                );
            }
            BrickData::Byte(if mode.fix() { rounded } else { stretched })
        }
    }
}

/// The output's storage types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Byte,
    Short,
    Float,
}

/// Sub-brick `iv` of `ds` as floats (`mri_scale_to_float(DSET_BRICK_FACTOR, DSET_BRICK)`).
fn brick(ds: &Dataset, iv: usize) -> Vec<f32> {
    let nvox = ds.nxyz.iter().product();
    automask::scale_to_float(&ds.data, nvox, iv, ds.factors[iv])
}

/// Runs 3dvolreg's registration on `ds` (loaded). `null_output`: `-prefix NULL`. Messages go to
/// `log` in AFNI's order; the upstream defects met to `notices`.
///
/// Memory, as AFNI holds it (`DSET_unload_one` after each registered sub-brick): with an output,
/// `ds`'s data become the output's ([`VolregResult::output`]) and `ds` is left without data. Each
/// registered sub-brick is written over its input sub-brick when both are stored alike, else the
/// input is held sub-brick by sub-brick and each freed once registered.
#[allow(clippy::too_many_lines, clippy::needless_range_loop)] // AFNI's index loops, as written
pub fn volreg(
    ds: &mut Dataset,
    p: &VolregParams,
    null_output: bool,
    log: &mut Vec<Message>,
    notices: &mut Notices,
) -> Result<VolregResult, VolregError> {
    let o = orient(ds)?;
    let n = ds.nxyz;
    let [nx, ny, nz] = n;
    let nxyz = nx * ny * nz;
    let imcount = ds.nvals;
    let mode = p.mode;
    let verbose = p.verbose > 0;
    let raw = |s: String, log: &mut Vec<Message>| log.push(Message::Raw(s));
    let del = ds.geometry.axes.del;
    let d = del.map(f32::abs);

    let iha = handedness(o);
    let (ax1, ax2, ax3) = (axcode(o, 'I'), axcode(o, 'R'), axcode(o, 'A'));
    let (hax1, hax2, hax3) = (ax1 * iha, ax2 * iha, ax3 * iha);
    let axes = [ax1.abs() - 1, ax2.abs() - 1, ax3.abs() - 1];

    // The base.
    let (imbase, nbase): (Vec<f32>, Option<usize>) = match &p.base {
        Base::Index(k) => (brick(ds, *k), Some(*k)),
        Base::External { vol, .. } => (vol.clone(), None),
    };
    let nnz = nonzero_count(&imbase);
    if nnz < 100 {
        return Err(VolregError::Fatal(format!(
            "3dvolreg fails: base image has {nnz} nonzero voxel{} (< 100)",
            if nnz == 1 { "" } else { "s" }
        )));
    }
    let base_vol = Vol {
        n,
        d,
        data: imbase.clone(),
    };

    // The edging.
    let (xf, yf, zf) = match p.edging {
        Edging::Voxels(e) => (
            (0.25 * nx as f64).min(f64::from(e)) as i32,
            (0.25 * ny as f64).min(f64::from(e)) as i32,
            (0.25 * nz as f64).min(f64::from(e)) as i32,
        ),
        Edging::Percent(e) => (
            (0.01 * f64::from(e) * nx as f64 + 0.5) as i32,
            (0.01 * f64::from(e) * ny as f64 + 0.5) as i32,
            (0.01 * f64::from(e) * nz as f64 + 0.5) as i32,
        ),
    };
    let mut al = Align3d {
        edge: [xf, yf, zf],
        wtrim: true,
        ..Align3d::default()
    };
    if verbose {
        raw(format!("++ Edging: x={xf} y={yf} z={zf}\n"), log);
    }

    // -maxdisp: the boundary voxels of the automask (clip fraction 0.333).
    let mut dispvec: Vec<FVec3> = Vec::new();
    let mut maxdisp = p.maxdisp;
    if p.maxdisp {
        if verbose {
            log.push(Message::Info("Creating mask for -maxdisp".to_owned()));
        }
        let mut ms = MaskSettings::default();
        ms.set_clipfrac(0.333);
        let mut mlog = Vec::new();
        match automask::automask(ds, &ms, mode, &mut mlog) {
            Some(dsk) => {
                if verbose {
                    log.push(Message::InInfo(format!(
                        "Automask has {} voxels",
                        mask::count(&dsk)
                    )));
                }
                let nxy = nx * ny;
                let on = |i: usize, j: usize, k: usize| dsk[i + j * nx + k * nxy] != 0;
                let mut msk = vec![0u8; nxyz];
                let mut nmsk = 0usize;
                for mm in 0..nxyz {
                    if dsk[mm] == 0 {
                        continue;
                    }
                    let (ii, kk, jj) = (mm % nx, mm / nxy, (mm % nxy) / nx);
                    if ii + 1 >= nx || ii == 0 || jj + 1 >= ny || jj == 0 || kk + 1 >= nz || kk == 0
                    {
                        msk[mm] = 1;
                        nmsk += 1;
                        continue;
                    }
                    if on(ii + 1, jj, kk)
                        && on(ii - 1, jj, kk)
                        && on(ii, jj + 1, kk)
                        && on(ii, jj - 1, kk)
                        && on(ii, jj, kk + 1)
                        && on(ii, jj, kk - 1)
                    {
                        continue;
                    }
                    msk[mm] = 1;
                    nmsk += 1;
                }
                if verbose {
                    log.push(Message::InInfo(format!(
                        "{nmsk} voxels left in -maxdisp mask after erosion"
                    )));
                }
                maxdisp = nmsk > 0;
                let ax = &ds.geometry.axes;
                for mm in 0..nxyz {
                    if msk[mm] == 0 {
                        continue;
                    }
                    let (ii, kk, jj) = (mm % nx, mm / nxy, (mm % nxy) / nx);
                    dispvec.push([
                        ax.org[0] + ii as f32 * ax.del[0],
                        ax.org[1] + jj as f32 * ax.del[1],
                        ax.org[2] + kk as f32 * ax.del[2],
                    ]);
                }
            }
            None => {
                maxdisp = false;
                log.push(Message::Warning("Can't create -maxdisp mask?!".to_owned()));
            }
        }
    }
    // AFNI-92: the clip fraction THD_automask keeps for the coarse rotation search's mask.
    let clfrac = if p.maxdisp || mode.fix() {
        0.333f32
    } else {
        0.5
    };
    if mode.reproduce() && !p.maxdisp && p.twopass && p.coarse_rot {
        notices.add(
            &defects::AFNI_92,
            "-nomaxdisp leaves the automask's clip fraction 0.5 for -twopass's coarse rotation \
             search, where the default (-maxdisp) sets 0.333: the registration depends on \
             -nomaxdisp, as in AFNI",
        );
    } else if mode.fix() && !p.maxdisp && p.twopass && p.coarse_rot {
        notices.add(
            &defects::AFNI_92,
            "-nomaxdisp: the coarse rotation search's automask keeps clip fraction 0.333, as with \
             the default -maxdisp; AFNI uses 0.5 then, so its registration depends on \
             -nomaxdisp",
        );
    }

    let mut rot = Rotator::new(p.resam, p.zpad, mode);
    // AFNI-97 met by a Fourier rotation (the default's notice; `reproduce` stops there).
    let mut null_rows = false;
    let mut blur = Blur::new(p.firfac, p.firold, mode);
    blur.whole_point_mirrors = p.whole_point_mirrors;
    let mut ctx = Ctx {
        p,
        o,
        n,
        d,
        notices,
    };

    let mut first: Vec<[f32; 6]> = vec![[0.0; 6]; imcount];
    if p.twopass {
        if verbose {
            raw(
                "++ Start of first pass alignment on all sub-bricks\n".to_owned(),
                log,
            );
        }
        let mut tp_base = imbase.clone();
        crate::dataset::floatscan(&mut tp_base);
        blur.blur_volume_3d(n, [1.0; 3], &mut tp_base, [p.twoblur; 3]);
        if let Some(m) = blur.take_note() {
            log.push(Message::Info(m));
        }
        al.params(
            p.maxite,
            p.twoblur * p.dxy,
            p.twoblur * p.dph,
            p.twoblur * p.del,
            axes,
            -1,
        );
        al.method(Method::Linear, p.verbose > 1, true, false);
        let mut sx = [66666i32, 0, 0];
        let mut ang = [0.0f32; 3];
        let mut coarse = Coarse {
            del: p.coarse_del,
            num: p.coarse_num,
        };
        let tp_vol = Vol {
            n,
            d,
            data: tp_base.clone(),
        };
        let basis = if p.weight.is_some() {
            let w = p.weight.as_ref().map(|w| Vol {
                n,
                d,
                data: w.clone(),
            });
            if nz < 2 {
                return Err(not_3d(log, "first pass alignment algorithm"));
            }
            al.setup(&tp_vol, w.as_ref(), &mut rot, &mut blur, log)
        } else {
            if verbose {
                raw(
                    "++ Computing first pass weight as sum of base and brick\n".to_owned(),
                    log,
                );
            }
            let mut far = brick(ds, 0);
            blur.blur_volume_3d(n, [1.0; 3], &mut far, [p.twoblur; 3]);
            if let Some(m) = blur.take_note() {
                log.push(Message::Info(m));
            }
            if coarse.del > 0 && coarse.num > 0 {
                if !p.coarse_rot {
                    if verbose {
                        raw("++ Getting best coarse shift [0]:".to_owned(), log);
                    }
                    let mut met = false;
                    let (_, s) = best_shift(n, &tp_base, &far, None, &coarse, mode.fix(), &mut met);
                    note_empty(&mut ctx, met);
                    sx = s;
                    if verbose {
                        raw(format!(" {} {} {}\n", sx[0], sx[1], sx[2]), log);
                    }
                } else {
                    if verbose {
                        raw("++ Getting best coarse rot+shift [0]:".to_owned(), log);
                    }
                    let (a, s) = best_shiftrot(
                        &mut ctx,
                        &tp_base,
                        &far,
                        &mut coarse,
                        &mut rot,
                        &blur,
                        clfrac,
                        log,
                    )?;
                    ang = a;
                    sx = s;
                    if hax1 < 0 {
                        ang[0] = -ang[0];
                    }
                    if hax2 < 0 {
                        ang[1] = -ang[1];
                    }
                    if hax3 < 0 {
                        ang[2] = -ang[2];
                    }
                    if verbose {
                        raw(
                            format!(
                                " {} {} {} : {} {} {}\n",
                                g(ang[0]),
                                g(ang[1]),
                                g(ang[2]),
                                sx[0],
                                sx[1],
                                sx[2]
                            ),
                            log,
                        );
                    }
                }
            } else {
                sx = [0, 0, 0];
                ang = [0.0; 3];
            }
            let mut qar = tp_base.clone();
            let lo = sx.map(|e| e.unsigned_abs() as usize);
            let at = |i: usize, j: usize, k: usize| i + j * nx + k * nx * ny;
            let shifted = |i: usize, j: usize, k: usize| {
                at(
                    (i as i32 - sx[0]) as usize,
                    (j as i32 - sx[1]) as usize,
                    (k as i32 - sx[2]) as usize,
                )
            };
            if p.sinit {
                let (mut sf, mut sq) = (0.0f32, 0.0f32);
                for kk in lo[2]..nz.saturating_sub(lo[2]) {
                    for jj in lo[1]..ny.saturating_sub(lo[1]) {
                        for ii in lo[0]..nx.saturating_sub(lo[0]) {
                            sf += far[shifted(ii, jj, kk)];
                            sq += qar[at(ii, jj, kk)];
                        }
                    }
                }
                if sq > 0.0 {
                    sf /= sq;
                    if f64::from(sf) > 0.005 && f64::from(sf) < 2000.0 {
                        al.scaleinit(sf);
                        if f64::from(sf) < 200.0 {
                            if verbose {
                                raw(format!("++ Scale init = {}\n", g(sf)), log);
                            }
                        } else {
                            raw(
                                format!(
                                    "++ Warning: Scale init = {} is large. Check output.\n",
                                    g(sf)
                                ),
                                log,
                            );
                        }
                    } else {
                        raw(
                            format!(
                                "-- Large scale difference between datasets.\n   Scale init = \
                                 {}\n   3dvolreg might not converge.",
                                g(sf)
                            ),
                            log,
                        );
                    }
                }
            }
            for kk in lo[2]..nz.saturating_sub(lo[2]) {
                for jj in lo[1]..ny.saturating_sub(lo[1]) {
                    for ii in lo[0]..nx.saturating_sub(lo[0]) {
                        qar[at(ii, jj, kk)] += far[shifted(ii, jj, kk)];
                    }
                }
            }
            if verbose {
                raw("++ Blurring first pass weight\n".to_owned(), log);
            }
            blur.blur_volume_3d(n, [1.0; 3], &mut qar, [p.twoblur; 3]);
            let clip = (0.025 * mri_max(&qar)) as f32;
            for x in &mut qar {
                if *x < clip {
                    *x = 0.0;
                }
            }
            al.force_edging = true;
            let w = Vol { n, d, data: qar };
            if nz < 2 {
                return Err(not_3d(log, "first pass alignment algorithm"));
            }
            let b = al.setup(&tp_vol, Some(&w), &mut rot, &mut blur, log);
            al.force_edging = false;
            b
        };
        for kim in 0..imcount {
            let mut fim = brick(ds, kim);
            if Some(kim) != nbase && nonzero_count(&fim) >= 66 {
                blur.blur_volume_3d(n, [1.0; 3], &mut fim, [p.twoblur; 3]);
                if kim > 0 || sx[0] == 66666 {
                    if coarse.del > 0 && coarse.num > 0 {
                        if !p.coarse_rot {
                            if verbose {
                                raw(format!("++ Getting best coarse shift [{kim}]:"), log);
                            }
                            let mut met = false;
                            let (_, s) =
                                best_shift(n, &tp_base, &fim, None, &coarse, mode.fix(), &mut met);
                            note_empty(&mut ctx, met);
                            sx = s;
                            if verbose {
                                raw(format!(" {} {} {}\n", sx[0], sx[1], sx[2]), log);
                            }
                        } else {
                            if verbose {
                                raw(format!("++ Getting best coarse rot+shift [{kim}]:"), log);
                            }
                            let (a, s) = best_shiftrot(
                                &mut ctx,
                                &tp_base,
                                &fim,
                                &mut coarse,
                                &mut rot,
                                &blur,
                                clfrac,
                                log,
                            )?;
                            ang = a;
                            sx = s;
                            if hax1 < 0 {
                                ang[0] = -ang[0];
                            }
                            if hax2 < 0 {
                                ang[1] = -ang[1];
                            }
                            if hax3 < 0 {
                                ang[2] = -ang[2];
                            }
                            if verbose {
                                raw(
                                    format!(
                                        " {} {} {} : {} {} {}\n",
                                        g(ang[0]),
                                        g(ang[1]),
                                        g(ang[2]),
                                        sx[0],
                                        sx[1],
                                        sx[2]
                                    ),
                                    log,
                                );
                            }
                        }
                    } else {
                        sx = [0, 0, 0];
                        ang = [0.0; 3];
                    }
                }
                al.init = [
                    ang[0],
                    ang[1],
                    ang[2],
                    sx[0] as f32 * d[0],
                    sx[1] as f32 * d[1],
                    sx[2] as f32 * d[2],
                ];
                let im = Vol { n, d, data: fim };
                let (th, sh, _) = al
                    .one(&basis, &im, &mut rot, log, &mut null_rows)
                    .map_err(|e| one_failed(e, ctx.notices))?;
                first[kim] = [
                    (f64::from(th[0]) * (180.0 / PI)) as f32,
                    (f64::from(th[1]) * (180.0 / PI)) as f32,
                    (f64::from(th[2]) * (180.0 / PI)) as f32,
                    sh[0],
                    sh[1],
                    sh[2],
                ];
            }
        }
        if verbose {
            raw("++ CPU time for first pass=0 s\n".to_owned(), log);
        }
    }

    // The final alignment.
    if verbose {
        raw("++ Initializing alignment base\n".to_owned(), log);
    }
    al.params(p.maxite, p.dxy, p.dph, p.del, axes, -1);
    // 14 Feb 2001: with a final transformation (-rotparent) the registration makes no output.
    al.method(p.resam, p.verbose > 1, p.matvec.is_some(), p.clipit);
    let final_mode = p.final_mode.unwrap_or(p.resam);
    al.final_regmode = Some(final_mode);
    let mut weight = p.weight.clone();
    if p.wtinp {
        weight = Some(brick(ds, 0));
        al.wproc = true;
    }
    let wvol = weight.map(|w| Vol { n, d, data: w });
    if nz < 2 {
        return Err(not_3d(log, "base image for alignment"));
    }
    let basis: Basis = al.setup(&base_vol, wvol.as_ref(), &mut rot, &mut blur, log);
    if verbose {
        raw(
            format!("++ Starting final pass on {imcount} sub-bricks: "),
            log,
        );
    }

    let kind = if p.floatize {
        Kind::Float
    } else {
        match ds.data {
            BrickData::Byte(_) => Kind::Byte,
            BrickData::Short(_) => Kind::Short,
            BrickData::Float(_) => Kind::Float,
        }
    };
    let mut res = VolregResult::default();
    // -gridparent's zero padding: the output has the padded number of slices.
    let out_nz = match &p.matvec {
        Some(mv) => nz + mv.npad[0] + mv.npad[1],
        None => nz,
    };
    let out_nxyz = nx * ny * out_nz;
    let mut out_data = OutputData::new(ds, kind, out_nxyz, null_output);
    for kim in 0..imcount {
        if verbose {
            raw(format!("{kim}"), log);
        }
        let fim = out_data.input_brick(ds, kim);
        let (th, sh, tim): ([f32; 3], [f32; 3], Vec<f32>);
        if Some(kim) != nbase && nonzero_count(&fim) >= 66 {
            if p.twopass {
                al.init = first[kim];
            }
            let im = Vol {
                n,
                d,
                data: fim.clone(),
            };
            let (t, s, out) = al
                .one(&basis, &im, &mut rot, log, &mut null_rows)
                .map_err(|e| one_failed(e, ctx.notices))?;
            th = t;
            sh = s;
            tim = out.unwrap_or_default();
        } else {
            if Some(kim) == nbase {
                let mut b = imbase.clone();
                crate::dataset::floatscan(&mut b);
                tim = b;
            } else {
                log.push(Message::Warning(format!(
                    "skipped registration for #{kim} because image is empty-ish"
                )));
                tim = fim.clone();
            }
            th = [0.0; 3];
            sh = [0.0; 3];
        }
        if verbose {
            raw(".".to_owned(), log);
        }
        let to_deg = |x: f32| (f64::from(x) * (180.0 / PI)) as f32;
        let mut roll = to_deg(th[0]);
        let mut pitch = to_deg(th[1]);
        let mut yaw = to_deg(th[2]);
        if hax1 < 0 {
            roll = -roll;
        }
        if hax2 < 0 {
            pitch = -pitch;
        }
        if hax3 < 0 {
            yaw = -yaw;
        }
        // The shifts in DICOM order (S, L, P), each axis by its orientation; AFNI leaves a
        // component its axes never set as it was (uninitialised memory: never the case for a
        // dataset with three distinct axes).
        let (mut ddx, mut ddy, mut ddz) = (0.0f32, 0.0f32, 0.0f32);
        for (axis, &v) in sh.iter().enumerate() {
            match o[axis] {
                ORI_R2L => ddy = v,
                ORI_L2R => ddy = -v,
                ORI_P2A => ddz = -v,
                ORI_A2P => ddz = v,
                ORI_I2S => ddx = v,
                ORI_S2I => ddx = -v,
                _ => {}
            }
        }
        res.roll.push(roll);
        res.pitch.push(pitch);
        res.yaw.push(yaw);
        res.dx.push(ddx);
        res.dy.push(ddy);
        res.dz.push(ddz);
        // -rotparent: the final transformation on top of this one, on the (zero-padded) input.
        let tim = match &p.matvec {
            None => tim,
            Some(mv) => {
                let vm1 = rotcom_matvec(o, [roll, pitch, yaw], [ddx, ddy, ddz], rot.libc);
                let vm2 = DVecMat {
                    mm: mv.rmat,
                    vv: mv.tvec,
                };
                let vmtot = vecmat::mul_vecmat(&vm2, &vm1);
                let (fimp, np) = if mv.npad != [0, 0] {
                    (
                        crate::rot3d::volpad(&fim, n, [0, 0, mv.npad[0]], [0, 0, mv.npad[1]]),
                        [nx, ny, nz + mv.npad[0] + mv.npad[1]],
                    )
                } else {
                    (fim.clone(), n)
                };
                rot.method = final_mode;
                let mut t = rot
                    .rota3d_matvec(&fimp, np, d, &vmtot.mm, &vmtot.vv)
                    .map_err(|e| VolregError::Fatal(e.to_string()))?;
                for m in rot.messages.drain(..) {
                    log.push(Message::Raw(m));
                }
                if rot.take_null_row() {
                    null_rows = true;
                    if mode.reproduce() {
                        return Err(one_failed(align3d::OneError::NullRow, ctx.notices));
                    }
                }
                if p.clipit && align3d::clips(final_mode) {
                    let ftop = mri_max(&fimp) as f32;
                    let fbot = align3d::mri_min(&fimp) as f32;
                    for x in &mut t {
                        if *x < fbot {
                            *x = fbot;
                        } else if *x > ftop {
                            *x = ftop;
                        }
                    }
                }
                t
            }
        };
        // rmsnew, rmsold: float sums (0 with a final transformation: AFNI cannot compute them).
        let rms = |a: &[f32]| {
            let mut sum = 0.0f32;
            for (&b, &t) in imbase.iter().zip(a) {
                let q = b - t;
                sum += q * q;
            }
            (sum / a.len() as f32).sqrt()
        };
        if p.matvec.is_some() {
            res.rmsnew.push(0.0);
            res.rmsold.push(0.0);
        } else {
            res.rmsnew.push(rms(&tim));
            res.rmsold.push(rms(&fim));
        }
        if !null_output {
            out_data.put(ds, kim, store(&tim, kind, mode, ctx.notices));
        }
        if verbose {
            raw(".".to_owned(), log);
        }
    }

    if verbose {
        raw("\n++ CPU time for realignment=0 s".to_owned(), log);
        if imcount > 1 {
            raw("  [=0 s/sub-brick]".to_owned(), log);
        }
        raw("\n".to_owned(), log);
        summary(&res, imcount, log);
    }

    if null_rows {
        ctx.notices.add(
            &defects::AFNI_97,
            "a Fourier rotation shifted the last of an odd count of rows, with the previous \
             pair's second shift, beyond the row length: that row is cleared alone; AFNI clears \
             the missing second row through a NULL pointer and crashes",
        );
    }

    // The matrices and displacements.
    let cen_inp = dataset_center(ds)?;
    let cen_bas = match &p.base {
        Base::External { center, .. } => *center,
        Base::Index(_) => cen_inp,
    };
    let libc = vecmat::Libc::new(mode);
    let mut md = MaxDisp::default();
    let mut emaxvec: Vec<DVec3> = vec![[0.0; 3]; dispvec.len()];
    let mut emaxvec2: Vec<DVec3> = vec![[0.0; 3]; dispvec.len()];
    for kim in 0..imcount {
        let rmat = vecmat::rot_to_matrix(
            2,
            -(PI / 180.0) * f64::from(res.roll[kim]),
            0,
            -(PI / 180.0) * f64::from(res.pitch[kim]),
            1,
            -(PI / 180.0) * f64::from(res.yaw[kim]),
            libc,
        );
        let mut matar = [0.0f32; 12];
        for i in 0..3 {
            for j in 0..3 {
                matar[4 * i + j] = rmat[i][j] as f32;
            }
        }
        matar[3] = res.dy[kim];
        matar[7] = res.dz[kim];
        matar[11] = res.dx[kim];
        res.matvec.push(matar);
        res.rmat.push(rmat);
        {
            let dv = vecmat::matvec_f(&rmat, &cen_inp);
            let xd = matar[3] + (cen_bas[0] - dv[0]);
            let yd = matar[7] + (cen_bas[1] - dv[1]);
            let zd = matar[11] + (cen_bas[2] - dv[2]);
            let vm = DVecMat {
                mm: rmat,
                vv: [f64::from(xd), f64::from(yd), f64::from(zd)],
            };
            let ivm = vecmat::invert_vecmat(&vm);
            let mut row = [0.0f64; 12];
            for i in 0..3 {
                row[4 * i] = ivm.mm[i][0];
                row[4 * i + 1] = ivm.mm[i][1];
                row[4 * i + 2] = ivm.mm[i][2];
                row[4 * i + 3] = ivm.vv[i];
            }
            res.matrix_save.push(row);
        }
        if maxdisp {
            let tvec0: DVec3 = [
                f64::from(matar[3]),
                f64::from(matar[7]),
                f64::from(matar[11]),
            ];
            // AFNI-99: AFNI's signed conversion with `reproduce`, the permutation by default;
            // the other one is computed too, to name the defect where it changes a value.
            let convert = |pp: DMat33| {
                let ppt = vecmat::transpose(&pp);
                let r = vecmat::mul(&vecmat::mul(&ppt, &rmat), &pp);
                (r, vecmat::matvec(&ppt, &tvec0))
            };
            let (r, tvec) = convert(disp_conversion(o, mode));
            let other = has_negative_axis(o).then(|| {
                convert(disp_conversion(
                    o,
                    if mode.reproduce() {
                        UpstreamDefects::Fix
                    } else {
                        UpstreamDefects::Reproduce
                    },
                ))
            });
            let ax = &ds.geometry.axes;
            let centre = |a: usize| {
                (f64::from(ax.org[a]) + 0.5 * (n[a] as f64 - 1.0) * f64::from(ax.del[a])) as f32
            };
            let vorg: DVec3 = [
                f64::from(centre(0)),
                f64::from(centre(1)),
                f64::from(centre(2)),
            ];
            let (mut dmax, mut emax) = (0.0f32, 0.0f32);
            let (mut dmax2, mut emax2) = (0.0f32, 0.0f32);
            let disp = |r: &DMat33, t: &DVec3, dv: &DVec3| {
                let mut qv = vecmat::sub(dv, &vorg);
                qv = vecmat::matvec_add(r, &qv, t);
                qv = vecmat::add(&qv, &vorg);
                vecmat::sub(dv, &qv)
            };
            for (qq, dvf) in dispvec.iter().enumerate() {
                let dv: DVec3 = dvf.map(f64::from);
                let qv = disp(&r, &tvec, &dv);
                let ddd = vecmat::size(&qv) as f32;
                if ddd > dmax {
                    dmax = ddd;
                }
                let ev = if kim > 0 { emaxvec[qq] } else { qv };
                emaxvec[qq] = qv;
                let ev = vecmat::sub(&ev, &qv);
                let eee = vecmat::size(&ev) as f32;
                if eee > emax {
                    emax = eee;
                }
                if let Some((r2, t2)) = &other {
                    let qv2 = disp(r2, t2, &dv);
                    let d2 = vecmat::size(&qv2) as f32;
                    if d2 > dmax2 {
                        dmax2 = d2;
                    }
                    let ev2 = if kim > 0 { emaxvec2[qq] } else { qv2 };
                    emaxvec2[qq] = qv2;
                    let e2 = vecmat::size(&vecmat::sub(&ev2, &qv2)) as f32;
                    if e2 > emax2 {
                        emax2 = e2;
                    }
                }
            }
            if other.is_some() {
                // AFNI-99 is named by the command line where a printed value differs
                // (`notice_afni_99` in `cli::volreg`): the messages' two decimals and
                // -maxdisp1D's three hide most last-bit differences.
                let m2 = md.other.get_or_insert_with(Box::default);
                if m2.top.0 < dmax2 {
                    m2.top = (dmax2, kim);
                }
                if m2.top_delta.0 < emax2 {
                    m2.top_delta = (emax2, kim);
                }
                m2.dmax.push(dmax2);
                m2.emax.push(emax2);
            }
            if md.top.0 < dmax {
                md.top = (dmax, kim);
            }
            if md.top_delta.0 < emax {
                md.top_delta = (emax, kim);
            }
            md.dmax.push(dmax);
            md.emax.push(emax);
        }
    }
    if maxdisp {
        res.maxdisp = Some(md);
    }

    if !null_output {
        let mut out = header_of(ds);
        out.nxyz[2] = out_nz;
        out.data = out_data.finish(ds);
        out.factors = vec![0.0; imcount];
        res.output = Some(out);
    }
    Ok(res)
}

/// A dataset's header without its data.
fn header_of(ds: &Dataset) -> Dataset {
    Dataset {
        nxyz: ds.nxyz,
        nvals: ds.nvals,
        data: BrickData::Float(Vec::new()),
        factors: ds.factors.clone(),
        taxis: ds.taxis.clone(),
        geometry: ds.geometry.clone(),
        dropped_inter: ds.dropped_inter,
    }
}

/// Where the final pass reads its input sub-bricks and writes the registered ones (see
/// [`volreg`]'s memory note).
enum OutputData {
    /// `-prefix NULL`: the input stays, nothing is written.
    None,
    /// Each registered sub-brick over its input one (the same storage type and grid).
    InPlace,
    /// The input sub-brick by sub-brick (each freed once read: `DSET_unload_one`), the output
    /// in its own array of the output's type.
    Split {
        input: Vec<Option<BrickData>>,
        output: BrickData,
    },
}

impl OutputData {
    fn new(ds: &mut Dataset, kind: Kind, out_nxyz: usize, null_output: bool) -> OutputData {
        if null_output {
            return OutputData::None;
        }
        let nxyz: usize = ds.nxyz.iter().product();
        let same = matches!(
            (&ds.data, kind),
            (BrickData::Byte(_), Kind::Byte)
                | (BrickData::Short(_), Kind::Short)
                | (BrickData::Float(_), Kind::Float)
        );
        if same && out_nxyz == nxyz {
            return OutputData::InPlace;
        }
        let total = out_nxyz * ds.nvals;
        let output = match kind {
            Kind::Byte => BrickData::Byte(Vec::with_capacity(total)),
            Kind::Short => BrickData::Short(Vec::with_capacity(total)),
            Kind::Float => BrickData::Float(Vec::with_capacity(total)),
        };
        // The input's sub-bricks apart, so that each is freed once registered (the copy is
        // made before the output holds anything).
        let input = match std::mem::replace(&mut ds.data, BrickData::Float(Vec::new())) {
            BrickData::Byte(v) => v
                .chunks(nxyz)
                .map(|c| Some(BrickData::Byte(c.to_vec())))
                .collect(),
            BrickData::Short(v) => v
                .chunks(nxyz)
                .map(|c| Some(BrickData::Short(c.to_vec())))
                .collect(),
            BrickData::Float(v) => v
                .chunks(nxyz)
                .map(|c| Some(BrickData::Float(c.to_vec())))
                .collect(),
        };
        OutputData::Split { input, output }
    }

    /// Sub-brick `kim` of the input as floats (`mri_scale_to_float`); in a split input it is
    /// freed here (`DSET_unload_one`).
    fn input_brick(&mut self, ds: &Dataset, kim: usize) -> Vec<f32> {
        match self {
            OutputData::Split { input, .. } => {
                let b = input[kim].take().expect("each sub-brick is read once");
                let nvox = b.len();
                automask::scale_to_float(&b, nvox, 0, ds.factors[kim])
            }
            _ => brick(ds, kim),
        }
    }

    /// Stores registered sub-brick `kim` (`b`, of the output's type).
    fn put(&mut self, ds: &mut Dataset, kim: usize, b: BrickData) {
        match self {
            OutputData::None => {}
            OutputData::InPlace => {
                let n = b.len();
                let range = kim * n..(kim + 1) * n;
                match (&mut ds.data, b) {
                    (BrickData::Byte(v), BrickData::Byte(x)) => v[range].copy_from_slice(&x),
                    (BrickData::Short(v), BrickData::Short(x)) => v[range].copy_from_slice(&x),
                    (BrickData::Float(v), BrickData::Float(x)) => v[range].copy_from_slice(&x),
                    _ => unreachable!("an in-place output has the input's storage type"),
                }
            }
            OutputData::Split { output, .. } => match (output, b) {
                (BrickData::Byte(v), BrickData::Byte(x)) => v.extend_from_slice(&x),
                (BrickData::Short(v), BrickData::Short(x)) => v.extend_from_slice(&x),
                (BrickData::Float(v), BrickData::Float(x)) => v.extend_from_slice(&x),
                _ => unreachable!("the output array has the output's storage type"),
            },
        }
    }

    /// The output's data; `ds` is left without data (an in-place output is its data).
    fn finish(self, ds: &mut Dataset) -> BrickData {
        match self {
            OutputData::None => BrickData::Float(Vec::new()),
            OutputData::InPlace => std::mem::replace(&mut ds.data, BrickData::Float(Vec::new())),
            OutputData::Split { output, .. } => output,
        }
    }
}

/// AFNI-89: the least squares could not be set up (AFNI printed why) and AFNI dereferences its
/// result for the first brick it registers.
/// `mri_3dalign_setup` on a volume of one slice (not `MRI_IS_3D`): its message, then
/// 3dvolreg's "Can't initialize ..." and exit status 1.
fn not_3d(log: &mut Vec<Message>, what: &str) -> VolregError {
    log.push(Message::Raw(
        "\n*** mri_3dalign_setup: cannot use nD images!\x07\n".to_owned(),
    ));
    VolregError::Plain(format!("** Can't initialize {what}\n"))
}

/// Where AFNI crashes in `mri_3dalign_one`: AFNI-89's failed least squares, or AFNI-97's
/// Fourier shift of a missing row (`reproduce`; by default the rotation goes on).
fn one_failed(e: align3d::OneError, notices: &mut Notices) -> VolregError {
    match e {
        align3d::OneError::Lsq(_) => crash_after_lsqfit(notices),
        align3d::OneError::NullRow => {
            notices.add(
                &defects::AFNI_97,
                "a Fourier rotation shifted the last of an odd count of rows, with the previous \
                 pair's second shift, beyond the row length: AFNI clears the missing second row \
                 through a NULL pointer and crashes (segmentation fault) here",
            );
            VolregError::Crash(
                "a Fourier rotation shifted the last of an odd count of rows beyond the row \
                 length together with the previous pair's second shift (a rotation far beyond a \
                 head's motion); AFNI crashes here (AFNI-97; the default goes on)"
                    .to_owned(),
            )
        }
    }
}

fn crash_after_lsqfit(notices: &mut Notices) -> VolregError {
    notices.add(
        &defects::AFNI_89,
        "the registration's least squares could not be set up (see the messages above): AFNI \
         crashes (segmentation fault) at the first sub-brick it registers",
    );
    VolregError::Crash(
        "the base's least squares could not be set up (the messages above say why: all-zero \
         weights, or a base whose rotated and shifted copies do not change under the weight); \
         AFNI crashes here (AFNI-89)"
            .to_owned(),
    )
}

/// `"%g"` of a float.
fn g(x: f32) -> String {
    crate::cnum::fmt_g(f64::from(x))
}

/// The `-verbose` summary: the minimum, mean and maximum of each parameter.
fn summary(res: &VolregResult, imcount: usize, log: &mut Vec<Message>) {
    let cols: [&Vec<f32>; 6] = [&res.roll, &res.pitch, &res.yaw, &res.dx, &res.dy, &res.dz];
    let mut bot = [0.0f32; 6];
    let mut top = [0.0f32; 6];
    let mut bar = [0.0f32; 6];
    for kim in 0..imcount {
        for (c, col) in cols.iter().enumerate() {
            let v = col[kim];
            if kim == 0 {
                bot[c] = v;
                top[c] = v;
            } else {
                top[c] = if top[c] < v { v } else { top[c] };
                bot[c] = if bot[c] > v { v } else { bot[c] };
            }
            bar[c] += v;
        }
    }
    for b in &mut bar {
        *b /= imcount as f32;
    }
    let line = |label: &str, v: &[f32; 6]| {
        format!(
            "++ {label}: roll={:+.3}  pitch={:+.3}  yaw={:+.3}  dS={:+.3}  dL={:+.3}  dP={:+.3}\n",
            v[0], v[1], v[2], v[3], v[4], v[5]
        )
    };
    log.push(Message::Raw(line("Min ", &bot)));
    log.push(Message::Raw(line("Mean", &bar)));
    log.push(Message::Raw(line("Max ", &top)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_codes_and_handedness() {
        // RAI (DICOM order): x = R2L, y = A2P, z = I2S.
        let rai = [ORI_R2L, ORI_A2P, ORI_I2S];
        assert_eq!(axcode(rai, 'I'), 3);
        assert_eq!(axcode(rai, 'S'), -3);
        assert_eq!(axcode(rai, 'R'), 1);
        assert_eq!(axcode(rai, 'a'), 2);
        assert_eq!(handedness(rai), 1);
        // LPI: x = L2R, y = P2A, z = I2S.
        let lpi = [ORI_L2R, ORI_P2A, ORI_I2S];
        assert_eq!(axcode(lpi, 'R'), -1);
        assert_eq!(handedness(lpi), 1);
        let lai = [ORI_L2R, ORI_A2P, ORI_I2S];
        assert_eq!(handedness(lai), -1);
        assert_eq!(rotcom_value(1.23456), 1.2346f32);
        assert_eq!(rotcom_value(-9.0), -9.0);
    }

    #[test]
    fn voldif_of_an_empty_region() {
        let b = vec![1.0f32; 64];
        let (s, empty) = voldif([4, 4, 4], &b, [0, 0, 0], &b, 2, None);
        assert!(empty);
        assert_eq!(s, 0.0);
    }
}
