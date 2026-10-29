// SPDX-License-Identifier: Apache-2.0
//! The geometry AFNI reads from a NIfTI header and writes back (`specs/3dTshift.md` §6.1 and
//! §7.1: the matrix, its steps, the plumb test, the cardinal matrix, the quaternion, the
//! xform codes, the obliquity report).
//!
//! Two of AFNI's behaviours here are defects larmorx corrects by default (D13): a grid within
//! about 0.01° to 0.024° of plumb is written exactly plumb, its tilt discarded (AFNI-16), and
//! metre and micrometre units leave the offsets, or an oblique grid's whole matrix,
//! unconverted (AFNI-07). [`output_geometry`] follows AFNI with
//! [`UpstreamDefects::Reproduce`].
//!
//! The quaternion follows the method of the NIfTI reference library, nifti_clib (public
//! domain), in double precision, as the spec describes it; larmorx's single-precision port of
//! the same method for ITK is `larmorx_io::nifti::itk::write`. AFNI's x86-64 build evaluates a
//! few of its steps in x87 extended precision ([`crate::x87`]).

use larmorx_core::defects::{self, Notice, UpstreamDefects};
use larmorx_io::nifti::NiftiHeader;

use crate::cfmt;
use crate::msg::Message;
use crate::settings::AfniSettings;
use crate::x87;

/// A 3×4 matrix: rows x, y, z; columns i, j, k and the offsets.
pub type M34 = [[f32; 4]; 3];
type M33 = [[f64; 3]; 3];

/// Where AFNI took the matrix from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Sform,
    Qform,
    /// Neither: the voxel sizes on the diagonal, offsets 0.
    Pixdim,
}

/// The geometry of an input as AFNI reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct InputGeometry {
    /// The matrix, float32, in the file's units.
    pub m: M34,
    pub source: Source,
    /// The xform code of the transform used (0 without one).
    pub code: i32,
    /// `pixdim[1..3]` as float32, 1 where not positive.
    pub pixdim: [f32; 3],
    /// Millimetres per spatial unit: 1000 (metres), 0.001 (micrometres) or 1.
    pub mm_per_unit: f64,
    /// The code both output forms get (`qform_code` = `sform_code`).
    pub out_code: i32,
    /// For a qform in metres or micrometres, its matrix computed with the voxel sizes and
    /// offsets in millimetres (AFNI-07's default); `None` converts [`InputGeometry::m`] entry
    /// by entry.
    pub qform_mm: Option<M34>,
}

/// Millimetres per spatial unit of a header: 1000 for metres, 0.001 for micrometres, else 1.
pub fn millimetres_per_unit(hdr: &NiftiHeader) -> f64 {
    match hdr.xyzt_units & 0x07 {
        1 => 1000.0,
        3 => 0.001,
        _ => 1.0,
    }
}

/// A voxel size: the value as float32, 1 where not positive.
fn size(p: f64) -> f32 {
    let p = p as f32;
    if p > 0.0 { p } else { 1.0 }
}

/// The qform's matrix: the NIfTI-1 standard's matrix of the quaternion, computed in double
/// from the header's fields (spec §6.1), with the steps AFNI evaluates in extended precision
/// (§7.1), then rounded to float32.
pub fn qform_matrix(hdr: &NiftiHeader) -> M34 {
    qform_matrix_scaled(hdr, 1.0)
}

/// [`qform_matrix`] with the voxel sizes and offsets multiplied by `mm` in double before the
/// one rounding to float32: the matrix AFNI-07's correction writes for a qform in metres or
/// micrometres (observed by the clean-room implementer: the replica program's default on 100
/// qform-only headers in metres and micrometres, bit for bit; converting the float32 matrix
/// after its rounding differs in the last bit of most).
fn qform_matrix_scaled(hdr: &NiftiHeader, mm: f64) -> M34 {
    let (mut b, mut c, mut d) = (hdr.quatern_b, hdr.quatern_c, hdr.quatern_d);
    let s = (b * b + c * c) + d * d;
    let mut a = x87::add(1.0, -s);
    if a < 1.0e-7 {
        let r = x87::div(1.0, s.sqrt());
        b *= r;
        c *= r;
        d *= r;
        a = 0.0;
    } else {
        a = a.sqrt();
    }
    let xd = f64::from(size(hdr.pixdim[1])) * mm;
    let yd = f64::from(size(hdr.pixdim[2])) * mm;
    let mut zd = f64::from(size(hdr.pixdim[3])) * mm;
    if hdr.pixdim[0] < 0.0 {
        zd = -zd;
    }
    let f = |v: f64| v as f32;
    // The offset as float32, then (converted) times `mm` in double, rounded again.
    let offset = |v: f64| {
        let v = v as f32;
        if mm == 1.0 {
            v
        } else {
            (f64::from(v) * mm) as f32
        }
    };
    [
        [
            f((a * a + b * b - c * c - d * d) * xd),
            f(x87::mul(2.0 * (b * c - a * d), yd)),
            f(x87::mul(2.0 * (b * d + a * c), zd)),
            offset(hdr.qoffset_x),
        ],
        [
            f(x87::mul(2.0 * (b * c + a * d), xd)),
            f((a * a + c * c - b * b - d * d) * yd),
            f(x87::mul(2.0 * (c * d - a * b), zd)),
            offset(hdr.qoffset_y),
        ],
        [
            f(x87::mul(2.0 * (b * d - a * c), xd)),
            f(x87::mul(2.0 * (c * d + a * b), yd)),
            f((a * a + d * d - c * c - b * b) * zd),
            offset(hdr.qoffset_z),
        ],
    ]
}

/// The sform's rows as float32.
pub fn sform_matrix(hdr: &NiftiHeader) -> M34 {
    [hdr.srow_x, hdr.srow_y, hdr.srow_z].map(|row| row.map(|v| v as f32))
}

fn det33(m: &M34) -> f64 {
    let r = |i: usize, j: usize| f64::from(m[i][j]);
    r(0, 0) * (r(1, 1) * r(2, 2) - r(1, 2) * r(2, 1))
        - r(0, 1) * (r(1, 0) * r(2, 2) - r(1, 2) * r(2, 0))
        + r(0, 2) * (r(1, 0) * r(2, 1) - r(1, 1) * r(2, 0))
}

/// The view and template space of an xform code, and the code AFNI writes for them
/// (spec §7.1, "The output's xform codes"); a warning for a code NIfTI does not define.
fn output_code(code: i32, settings: &AfniSettings, messages: &mut Vec<Message>) -> i32 {
    let view_space = |view: &str| match view {
        "tlrc" => settings
            .get("AFNI_DEFAULT_STD_SPACE")
            .unwrap_or("TLRC")
            .to_owned(),
        "acpc" => "ACPC".to_owned(),
        _ => "ORIG".to_owned(),
    };
    let named_view = || {
        let v = settings.get("AFNI_NIFTI_VIEW").map(str::to_ascii_lowercase);
        match v.as_deref() {
            Some(w @ ("tlrc" | "orig" | "acpc")) => w.to_owned(),
            _ => "orig".to_owned(),
        }
    };
    let space = match code {
        0 | 1 => "ORIG".to_owned(),
        3 => "TLRC".to_owned(),
        4 => "MNI".to_owned(),
        5 => view_space("tlrc"),
        2 => view_space(&named_view()),
        _ => {
            messages.push(Message::Warning(format!(
                "I don't think this sform code '{code}' is allowed!"
            )));
            view_space(&named_view())
        }
    };
    // AFNI 25.2.09's table of template spaces (AFNI_atlas_spaces.niml): the generic space.
    match space.as_str() {
        "TT_N27" | "TT_avg" | "TLRC" => 3,
        "MNI_152" | "MNI" | "MNI_SPM2" | "MNI_FSL" | "MNI_OTHER" | "MNI_2009c_asym" | "MNI_N27" => {
            4
        }
        "ORIG" | "ACPC" => 1,
        _ => 5,
    }
}

/// The geometry AFNI reads from `hdr` (spec §6.1, §7.1), and its messages in AFNI's order:
/// an illegal `AFNI_NIFTI_PRIORITY`, a qform and an sform of different handedness, no
/// transform at all, an xform code NIfTI does not define, an oblique grid. `shown` is the
/// input's name in messages.
pub fn read_geometry(
    hdr: &NiftiHeader,
    settings: &AfniSettings,
    shown: &str,
) -> (InputGeometry, Vec<Message>) {
    let mut messages = Vec::new();
    let prefer_q = match settings.get("AFNI_NIFTI_PRIORITY") {
        None => false,
        Some(v) if v.starts_with(['S', 's']) => false,
        Some(v) if v.starts_with(['Q', 'q']) => true,
        Some(v) => {
            messages.push(Message::Warning(format!(
                "Illegal AFNI_NIFTI_PRIORITY='{v}'"
            )));
            false
        }
    };
    let (has_q, has_s) = (hdr.qform_code > 0, hdr.sform_code > 0);
    if has_q && has_s {
        let (dq, ds) = (det33(&qform_matrix(hdr)), det33(&sform_matrix(hdr)));
        if dq * ds < 0.0 {
            let wins = if prefer_q { "Q" } else { "S" };
            messages.push(Message::Warning(format!(
                "NIfTI('{shown}'): Qform/Sform handedness differ; {wins} wins!"
            )));
        }
    }
    let pixdim = [
        size(hdr.pixdim[1]),
        size(hdr.pixdim[2]),
        size(hdr.pixdim[3]),
    ];
    let (m, source, code) = if has_s && !(prefer_q && has_q) {
        (sform_matrix(hdr), Source::Sform, hdr.sform_code)
    } else if has_q {
        (qform_matrix(hdr), Source::Qform, hdr.qform_code)
    } else {
        messages.push(Message::Warning(format!(
            "NO spatial transform (neither qform nor sform), in NIfTI file '{shown}'"
        )));
        // The offsets are -0, -0, 0 (observed: AFNI writes them so, spec §6.1's last example).
        let p = pixdim;
        (
            [
                [p[0], 0.0, 0.0, -0.0],
                [0.0, p[1], 0.0, -0.0],
                [0.0, 0.0, p[2], 0.0],
            ],
            Source::Pixdim,
            0,
        )
    };
    let out_code = output_code(code, settings, &mut messages);
    if !settings.yes("AFNI_NO_OBLIQUE_WARNING")
        && let Some(angle) = oblique_angle(&m)
    {
        messages.push(Message::Warning(format!(
            "  If you are performing spatial transformations on an oblique dset,\n  such as \
             {shown},\n  or viewing/combining it with volumes of differing obliquity,\n  you \
             should consider running: \n     3dWarp -deoblique \n  on this and  other oblique \
             datasets in the same session.\n See 3dWarp -help for details.\n"
        )));
        messages.push(Message::Info(format!(
            "Oblique dataset:{shown} is {} degrees from plumb.",
            cfmt::f(f64::from(angle))
        )));
    }
    let mm_per_unit = millimetres_per_unit(hdr);
    let qform_mm = (source == Source::Qform && mm_per_unit != 1.0)
        .then(|| qform_matrix_scaled(hdr, mm_per_unit));
    let geometry = InputGeometry {
        m,
        source,
        code,
        pixdim,
        mm_per_unit,
        out_code,
        qform_mm,
    };
    (geometry, messages)
}

/// AFNI's plumb test (spec §6.1, step 3): whether every column's largest entry over its
/// length rounds to 1 in float32, the row of each column's largest entry, and the smallest of
/// those ratios (the cosine of the largest tilt).
pub fn plumb_test(m: &M34) -> (bool, [usize; 3], f32) {
    let mut rows = [0usize; 3];
    let mut smallest = f32::INFINITY;
    let mut plumb = true;
    for j in 0..3 {
        let (x, y, z) = (m[0][j], m[1][j], m[2][j]);
        let s = (x * x + y * y) + z * z;
        let l = f64::from(s).sqrt() as f32;
        let mut row = 0;
        for i in 1..3 {
            if m[i][j].abs() > m[row][j].abs() {
                row = i;
            }
        }
        rows[j] = row;
        let r = (f64::from(m[row][j].abs()) / f64::from(l)) as f32;
        smallest = smallest.min(r);
        // NaN (a zero column) fails the test.
        if r.is_nan() || r < 1.0 {
            plumb = false;
        }
    }
    (plumb, rows, smallest)
}

/// The angle AFNI reports for an oblique grid (spec §7.1): `acos(m)·180/3.141592653`, from the
/// smallest cosine `m` of the plumb test, stored as a float; `None` at 0.01° or less.
pub fn oblique_angle(m: &M34) -> Option<f32> {
    let (plumb, _, smallest) = plumb_test(m);
    if plumb {
        return None;
    }
    // libm's acos (pure Rust): larmorx_core::math has no acos yet (rule 5). The result is
    // rounded to float32 and printed with 6 decimals. AFNI's π has 10 digits.
    #[allow(clippy::approx_constant)]
    const AFNI_PI: f64 = 3.141_592_653;
    let angle = (libm::acos(f64::from(smallest)) * 180.0 / AFNI_PI) as f32;
    (angle > 0.01).then_some(angle)
}

/// The geometry fields of an output header.
#[derive(Clone, Debug, PartialEq)]
pub struct OutputGeometry {
    /// `srow_x`, `srow_y`, `srow_z` (float32).
    pub srow: M34,
    /// `quatern_b`, `quatern_c`, `quatern_d`, as computed in double (NIfTI-1 rounds them).
    pub quatern: [f64; 3],
    /// `qfac` (`pixdim[0]`): 1 or −1.
    pub qfac: f64,
    /// `qoffset_x`, `qoffset_y`, `qoffset_z`.
    pub qoffset: [f32; 3],
    /// `pixdim[1..3]`: the steps, in millimetres.
    pub pixdim: [f32; 3],
}

/// The geometry AFNI writes for an input read as `g` (spec §6.1, step 4), and the notices of
/// AFNI-07 and AFNI-16 (named in both modes where the two modes' outputs differ).
pub fn output_geometry(g: &InputGeometry, mode: UpstreamDefects) -> (OutputGeometry, Vec<Notice>) {
    let mm = g.mm_per_unit;
    let convert = mm != 1.0;
    let mut notices = Vec::new();
    // The default converts the whole matrix first (AFNI-07 corrected).
    let m = if mode.fix() && convert {
        g.qform_mm
            .unwrap_or_else(|| g.m.map(|row| row.map(|v| (f64::from(v) * mm) as f32)))
    } else {
        g.m
    };
    // The steps in both modes: from the matrix in the file's units, then converted (spec §6.1,
    // step 2). Observed by the clean-room implementer: the replica program's default writes
    // these steps with the converted matrix (a 1 m column is a 1000 mm step, where the length
    // of the converted column can be 999.99994).
    let mut steps = match g.source {
        Source::Sform => [0, 1, 2].map(|j| {
            let (x, y, z) = (
                f64::from(g.m[0][j]),
                f64::from(g.m[1][j]),
                f64::from(g.m[2][j]),
            );
            ((x * x + y * y) + z * z).sqrt() as f32
        }),
        Source::Qform | Source::Pixdim => g.pixdim,
    };
    if convert {
        steps = steps.map(|s| (f64::from(s) * mm) as f32);
    }
    let (plumb, rows, _) = plumb_test(&m);
    let off_row =
        |m: &M34, rows: &[usize; 3]| (0..3).any(|j| (0..3).any(|i| i != rows[j] && m[i][j] != 0.0));
    let finite = m.iter().flatten().all(|v| v.is_finite());
    let tilt = plumb && finite && off_row(&m, &rows);
    // A plumb grid is written as its cardinal matrix, unless that discards a tilt and the
    // default keeps it (AFNI-16 corrected).
    let written = if plumb && !(mode.fix() && tilt) {
        let mut c: M34 = [[-0.0, -0.0, -0.0, 0.0], [-0.0, -0.0, -0.0, 0.0], [0.0; 4]];
        for j in 0..3 {
            let s = steps[j];
            c[rows[j]][j] = if m[rows[j]][j] < 0.0 { -s } else { s };
        }
        for i in 0..3 {
            c[i][3] = m[i][3];
        }
        c
    } else {
        m
    };
    // The notices, in the replica program's words and order (AFNI-07, then AFNI-16; observed
    // by the clean-room implementer).
    let mut tilt_notice = None;
    if tilt {
        // The largest angle between a column and its axis (double), and how far the plumb
        // matrix's entries are from the matrix it replaces (in reproduce, the file's matrix).
        let angle = (0..3)
            .map(|j| {
                let col = [0, 1, 2].map(|i| f64::from(m[i][j]));
                let len = ((col[0] * col[0] + col[1] * col[1]) + col[2] * col[2]).sqrt();
                (col[rows[j]].abs() / len).min(1.0).acos().to_degrees()
            })
            .fold(0.0f64, f64::max);
        let mut c: M34 = [[0.0; 4]; 3];
        for j in 0..3 {
            c[rows[j]][j] = if m[rows[j]][j] < 0.0 {
                -steps[j]
            } else {
                steps[j]
            };
        }
        let base = if mode.fix() { &m } else { &g.m };
        let apart = (0..3)
            .flat_map(|i| (0..3).map(move |j| (i, j)))
            .map(|(i, j)| (f64::from(c[i][j]) - f64::from(base[i][j])).abs())
            .fold(0.0f64, f64::max);
        let (a, d) = (cfmt::g(angle), cfmt::g(apart));
        let detail = match mode {
            UpstreamDefects::Fix => format!(
                "the input's matrix is {a} degrees from plumb: kept, in the sform and the qform; \
                 AFNI writes the plumb matrix of its axes instead (entries up to {d} apart)"
            ),
            UpstreamDefects::Reproduce => format!(
                "the input's matrix is {a} degrees from plumb: written as the plumb matrix of \
                 its axes, as AFNI does (entries up to {d} from the input's)"
            ),
        };
        tilt_notice = Some(Notice::new(&defects::AFNI_16, mode, detail));
    }
    if convert {
        let (file_plumb, file_rows, _) = plumb_test(&g.m);
        let offsets = (0..3).any(|i| g.m[i][3] != 0.0);
        if offsets || off_row(&g.m, &file_rows) {
            let unit = if mm > 1.0 { "metres" } else { "micrometres" };
            let detail = match (mode, file_plumb) {
                (UpstreamDefects::Fix, false) => format!(
                    "the input's units are {unit}: its oblique matrix and origin are converted \
                     to millimetres with its voxel sizes; AFNI converts only the voxel sizes and \
                     writes the matrix (sform) in {unit}"
                ),
                (UpstreamDefects::Fix, true) => format!(
                    "the input's units are {unit}: its origin is converted to millimetres with \
                     its voxel sizes; AFNI converts only the voxel sizes"
                ),
                (UpstreamDefects::Reproduce, false) => format!(
                    "the input's units are {unit}: its voxel sizes are converted to millimetres \
                     and its oblique matrix (sform) and origin are not, as AFNI does"
                ),
                (UpstreamDefects::Reproduce, true) => format!(
                    "the input's units are {unit}: its voxel sizes are converted to millimetres \
                     and its origin is not, as AFNI does"
                ),
            };
            notices.push(Notice::new(&defects::AFNI_07, mode, detail));
        }
    }
    notices.extend(tilt_notice);
    let linear: M33 = [0, 1, 2].map(|i| [0, 1, 2].map(|j| f64::from(written[i][j])));
    let (quatern, qfac) = quaternion(linear);
    let out = OutputGeometry {
        srow: written,
        quatern,
        qfac,
        qoffset: [written[0][3], written[1][3], written[2][3]],
        pixdim: steps,
    };
    (out, notices)
}

// ------------------------------------------------------------------------------------------------
// nifti_clib's quaternion of a matrix, in double precision (spec §6.1)

fn det(r: &M33) -> f64 {
    let [[r11, r12, r13], [r21, r22, r23], [r31, r32, r33]] = *r;
    r11 * r22 * r33 - r11 * r32 * r23 - r21 * r12 * r33 + r21 * r32 * r13 + r31 * r12 * r23
        - r31 * r22 * r13
}

/// The inverse by cofactors. The reciprocal of the determinant is taken in x87 extended
/// precision and stored as a double (spec §6.1, observed by the clean-room implementer on
/// 1,200 matrices whose quaternion AFNI wrote as doubles, NIfTI-2: the only way all 1,200
/// came out bit for bit; a direct double reciprocal missed 100, mostly in the rounding
/// residue of components that are 0 or of a rotation's `a` near 0, whose sign decides the
/// quaternion's).
fn inverse(r: &M33) -> M33 {
    let [[r11, r12, r13], [r21, r22, r23], [r31, r32, r33]] = *r;
    let mut deti = det(r);
    if deti != 0.0 {
        deti = x87::div(1.0, deti);
    }
    [
        [
            deti * (r22 * r33 - r32 * r23),
            deti * (-r12 * r33 + r32 * r13),
            deti * (r12 * r23 - r22 * r13),
        ],
        [
            deti * (-r21 * r33 + r31 * r23),
            deti * (r11 * r33 - r31 * r13),
            deti * (-r11 * r23 + r21 * r13),
        ],
        [
            deti * (r21 * r32 - r31 * r22),
            deti * (-r11 * r32 + r31 * r12),
            deti * (r11 * r22 - r21 * r12),
        ],
    ]
}

fn rownorm(a: &M33) -> f64 {
    let row = |i: usize| a[i][0].abs() + a[i][1].abs() + a[i][2].abs();
    let (mut r1, r2, r3) = (row(0), row(1), row(2));
    if r1 < r2 {
        r1 = r2;
    }
    if r1 < r3 {
        r1 = r3;
    }
    r1
}

fn colnorm(a: &M33) -> f64 {
    let col = |j: usize| a[0][j].abs() + a[1][j].abs() + a[2][j].abs();
    let (mut r1, r2, r3) = (col(0), col(1), col(2));
    if r1 < r2 {
        r1 = r2;
    }
    if r1 < r3 {
        r1 = r3;
    }
    r1
}

/// The orthogonal matrix nearest to `a` (polar decomposition by scaled Newton iteration).
fn polar(a: &M33) -> M33 {
    let mut x = *a;
    let mut dif = 1.0f64;
    let mut k = 0;
    let mut gam = det(&x);
    while gam == 0.0 {
        gam = 0.00001 * (0.001 + rownorm(&x));
        x[0][0] += gam;
        x[1][1] += gam;
        x[2][2] += gam;
        gam = det(&x);
    }
    loop {
        let y = inverse(&x);
        let (gam, gmi) = if dif > 0.3 {
            let alp = (rownorm(&x) * colnorm(&x)).sqrt();
            let bet = (rownorm(&y) * colnorm(&y)).sqrt();
            let gam = (bet / alp).sqrt();
            (gam, 1.0 / gam)
        } else {
            (1.0, 1.0)
        };
        let z: M33 =
            std::array::from_fn(|i| std::array::from_fn(|j| 0.5 * (gam * x[i][j] + gmi * y[j][i])));
        let mut sum = 0.0;
        for i in 0..3 {
            for j in 0..3 {
                sum += (z[i][j] - x[i][j]).abs();
            }
        }
        dif = sum;
        k += 1;
        if k > 100 || dif < 3.0e-6 {
            return z;
        }
        x = z;
    }
}

/// The NIfTI-1 quaternion `(b, c, d)` and `qfac` of a matrix's rotation, by nifti_clib's
/// method in double, with the quotients and the sum `1 + r11 + r22 + r33` in x87 extended
/// precision as AFNI's x86-64 build evaluates them (spec §6.1, §7.1).
pub fn quaternion(r: M33) -> ([f64; 3], f64) {
    let [
        [mut r11, mut r12, mut r13],
        [mut r21, mut r22, mut r23],
        [mut r31, mut r32, mut r33],
    ] = r;
    let mut xd = (r11 * r11 + r21 * r21 + r31 * r31).sqrt();
    let mut yd = (r12 * r12 + r22 * r22 + r32 * r32).sqrt();
    let mut zd = (r13 * r13 + r23 * r23 + r33 * r33).sqrt();
    if xd == 0.0 {
        (r11, r21, r31, xd) = (1.0, 0.0, 0.0, 1.0);
    }
    if yd == 0.0 {
        (r22, r12, r32, yd) = (1.0, 0.0, 0.0, 1.0);
    }
    if zd == 0.0 {
        (r33, r13, r23, zd) = (1.0, 0.0, 0.0, 1.0);
    }
    r11 /= xd;
    r21 /= xd;
    r31 /= xd;
    r12 /= yd;
    r22 /= yd;
    r32 /= yd;
    r13 /= zd;
    r23 /= zd;
    r33 /= zd;
    let p = polar(&[[r11, r12, r13], [r21, r22, r23], [r31, r32, r33]]);
    let [
        [r11, r12, mut r13],
        [r21, r22, mut r23],
        [r31, r32, mut r33],
    ] = p;
    let qfac = if det(&p) > 0.0 {
        1.0
    } else {
        r13 = -r13;
        r23 = -r23;
        r33 = -r33;
        -1.0
    };
    let mut a = x87::add(r11 + r22 + r33, 1.0);
    let (b, c, d);
    if a > 0.5 {
        a = 0.5 * a.sqrt();
        b = x87::div(0.25 * (r32 - r23), a);
        c = x87::div(0.25 * (r13 - r31), a);
        d = x87::div(0.25 * (r21 - r12), a);
    } else {
        let xd = 1.0 + r11 - (r22 + r33);
        let yd = 1.0 + r22 - (r11 + r33);
        let zd = 1.0 + r33 - (r11 + r22);
        let (mut bb, mut cc, mut dd);
        if xd > 1.0 {
            bb = 0.5 * xd.sqrt();
            cc = x87::div(0.25 * (r12 + r21), bb);
            dd = x87::div(0.25 * (r13 + r31), bb);
            a = x87::div(0.25 * (r32 - r23), bb);
        } else if yd > 1.0 {
            cc = 0.5 * yd.sqrt();
            bb = x87::div(0.25 * (r12 + r21), cc);
            dd = x87::div(0.25 * (r23 + r32), cc);
            a = x87::div(0.25 * (r13 - r31), cc);
        } else {
            dd = 0.5 * zd.sqrt();
            bb = x87::div(0.25 * (r13 + r31), dd);
            cc = x87::div(0.25 * (r23 + r32), dd);
            a = x87::div(0.25 * (r21 - r12), dd);
        }
        if a < 0.0 {
            bb = -bb;
            cc = -cc;
            dd = -dd;
        }
        (b, c, d) = (bb, cc, dd);
    }
    ([b, c, d], qfac)
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::element::DataType;
    use larmorx_io::nifti::NiftiVersion;

    const FIX: UpstreamDefects = UpstreamDefects::Fix;
    const REPRODUCE: UpstreamDefects = UpstreamDefects::Reproduce;

    fn header(rows: [[f64; 4]; 3], units: i32) -> NiftiHeader {
        let mut h = NiftiHeader::new(NiftiVersion::V1, &[2, 2, 2, 3], DataType::F32).unwrap();
        h.srow_x = rows[0];
        h.srow_y = rows[1];
        h.srow_z = rows[2];
        h.sform_code = 1;
        h.qform_code = 0;
        h.xyzt_units = units;
        h
    }

    fn bits(m: &M34) -> Vec<u32> {
        m.iter().flatten().map(|v| v.to_bits()).collect()
    }

    /// Spec §6.1's worked examples (observed).
    #[test]
    fn worked_examples() {
        let s = AfniSettings::new();
        // 0.006° about y, millimetres: plumb, written as the cardinal matrix.
        let h = header(
            [
                [2.0, 0.0, -0.0003, -7.0],
                [0.0, 2.0, 0.0, -5.0],
                [0.0002, 0.0, 3.0, -6.0],
            ],
            10,
        );
        let (g, _) = read_geometry(&h, &s, "in.nii");
        let (out, notices) = output_geometry(&g, REPRODUCE);
        let expected: M34 = [
            [2.0, -0.0, -0.0, -7.0],
            [-0.0, 2.0, -0.0, -5.0],
            [0.0, 0.0, 3.0, -6.0],
        ];
        assert_eq!(bits(&out.srow), bits(&expected));
        assert_eq!(
            out.quatern.map(f64::to_bits),
            [0.0, -0.0, 0.0f64].map(f64::to_bits)
        );
        assert_eq!(
            (out.qfac, out.qoffset, out.pixdim),
            (1.0, [-7.0, -5.0, -6.0], [2.0, 2.0, 3.0])
        );
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].defect.id, "AFNI-16");
        // Corrected: the matrix is kept.
        let (out, notices) = output_geometry(&g, FIX);
        assert_eq!(bits(&out.srow), bits(&g.m));
        assert_eq!(notices[0].defect.id, "AFNI-16");

        // Metres, sagittal, 0.0037° from plumb.
        let h = header(
            [
                [0.0, 0.0, -0.0024, 0.09],
                [0.0031, 0.0, 0.0, -0.12],
                [2e-7, -0.0031, 0.0, 0.05],
            ],
            1 | 8,
        );
        let (g, _) = read_geometry(&h, &s, "in.nii");
        let (out, _) = output_geometry(&g, REPRODUCE);
        let expected: M34 = [
            [-0.0, -0.0, -2.4, 0.09],
            [3.1000001, -0.0, -0.0, -0.12],
            [0.0, -3.1000001, 0.0, 0.05],
        ];
        assert_eq!(bits(&out.srow), bits(&expected));
        assert_eq!(out.quatern.map(|v| v as f32), [-0.5, -0.5, 0.5]);
        assert_eq!(out.pixdim, [3.1000001, 3.1000001, 2.4]);
        assert_eq!(out.qoffset, [0.09, -0.12, 0.05]);

        // Metres, oblique (cos 0.8 about y): the input's sform, in metres.
        let h = header(
            [
                [0.0024, 0.0, -0.0024, -0.1],
                [0.0, 0.003, 0.0, 0.05],
                [0.0018, 0.0, 0.0032, 0.02],
            ],
            1 | 8,
        );
        let (g, messages) = read_geometry(&h, &s, "in.nii");
        let (out, notices) = output_geometry(&g, REPRODUCE);
        assert_eq!(bits(&out.srow), bits(&g.m));
        assert_eq!(out.quatern.map(|v| v as f32), [0.0, -0.31622776, 0.0]);
        assert_eq!(out.pixdim, [3.0, 3.0, 4.0]);
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].defect.id, "AFNI-07");
        assert!(matches!(&messages[1], Message::Info(t) if t.contains(" degrees from plumb")));
        // Corrected: the whole matrix in millimetres.
        let (out, _) = output_geometry(&g, FIX);
        assert_eq!(out.srow[0], [2.4, 0.0, -2.4, -100.0]);

        // Neither form: the voxel sizes.
        let mut h = header([[0.0; 4]; 3], 10);
        h.sform_code = 0;
        h.pixdim[1..4].copy_from_slice(&[f64::from(3.1f32), f64::from(2.4f32), f64::from(0.8f32)]);
        let (g, messages) = read_geometry(&h, &s, "in.nii");
        assert!(matches!(&messages[0], Message::Warning(t) if t.starts_with("NO spatial")));
        let (out, _) = output_geometry(&g, REPRODUCE);
        let expected: M34 = [
            [3.1, -0.0, -0.0, -0.0],
            [-0.0, 2.4, -0.0, -0.0],
            [0.0, 0.0, 0.8, 0.0],
        ];
        assert_eq!(bits(&out.srow), bits(&expected));
        assert_eq!(
            out.qoffset.map(f32::to_bits),
            [-0.0f32, -0.0, 0.0].map(f32::to_bits)
        );
        h.xyzt_units = 1 | 8;
        h.pixdim[1..4].copy_from_slice(&[
            f64::from(0.0031f32),
            f64::from(0.0024f32),
            f64::from(0.0008f32),
        ]);
        let (g, _) = read_geometry(&h, &s, "in.nii");
        let (out, _) = output_geometry(&g, REPRODUCE);
        assert_eq!(out.pixdim, [3.1000001, 2.4, 0.79999995]);
    }

    /// Spec §6.1, step 3: the plumb test's boundary (observed with AFNI): the largest y entry
    /// of a column of step `s` (tilted toward y) that stays plumb, printed with `%g`.
    #[test]
    fn plumb_boundary() {
        let boundary = |s: f32| {
            let column =
                |y: f32| -> M34 { [[s, 0.0, 0.0, 0.0], [y, s, 0.0, 0.0], [0.0, 0.0, s, 0.0]] };
            // Bisect on the bits: plumb at s·1e-4, oblique at s·1e-3.
            let (mut lo, mut hi) = ((s * 1e-4).to_bits(), (s * 1e-3).to_bits());
            while hi - lo > 1 {
                let mid = lo + (hi - lo) / 2;
                if plumb_test(&column(f32::from_bits(mid))).0 {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            f32::from_bits(lo)
        };
        assert_eq!(cfmt::g32(boundary(2.0)), "0.000845728");
        assert_eq!(cfmt::g32(boundary(3.0)), "0.000690534");
        // AFNI's last plumb value; the next float, the first oblique one, prints 0.000172634.
        assert_eq!(boundary(0.8).to_bits(), 0x3935_04f3); // 0.000172633489
        // The oblique report for spec §7.1's example: 36.869896 degrees.
        let m: M34 = [
            [2.4, 0.0, -2.4, 0.0],
            [0.0, 3.0, 0.0, 0.0],
            [1.8, 0.0, 3.2, 0.0],
        ];
        assert_eq!(cfmt::f(f64::from(oblique_angle(&m).unwrap())), "36.869896");
    }

    #[test]
    fn xform_codes() {
        let code = |c: i32, pairs: &[(&str, &str)]| {
            let s = AfniSettings::from_pairs(pairs.iter().copied());
            let mut m = Vec::new();
            (output_code(c, &s, &mut m), m.len())
        };
        assert_eq!(code(0, &[]), (1, 0));
        assert_eq!(code(2, &[]), (1, 0));
        assert_eq!(code(3, &[]), (3, 0));
        assert_eq!(code(4, &[]), (4, 0));
        assert_eq!(code(5, &[]), (3, 0));
        assert_eq!(code(7, &[]), (1, 1));
        assert_eq!(code(2, &[("AFNI_NIFTI_VIEW", "TLRC")]), (3, 0));
        assert_eq!(code(2, &[("AFNI_NIFTI_VIEW", "tlrcx")]), (1, 0));
        assert_eq!(code(2, &[("AFNI_NIFTI_VIEW", "acpc")]), (1, 0));
        assert_eq!(code(7, &[("AFNI_NIFTI_VIEW", "tlrc")]), (3, 1));
        assert_eq!(code(5, &[("AFNI_DEFAULT_STD_SPACE", "MNI")]), (4, 0));
        assert_eq!(code(5, &[("AFNI_DEFAULT_STD_SPACE", "MNI_ANAT")]), (5, 0));
        assert_eq!(code(5, &[("AFNI_DEFAULT_STD_SPACE", "mni")]), (5, 0));
        assert_eq!(code(5, &[("AFNI_DEFAULT_STD_SPACE", "TT_N27")]), (3, 0));
        assert_eq!(code(5, &[("AFNI_DEFAULT_STD_SPACE", "ORIG")]), (1, 0));
        assert_eq!(code(5, &[("AFNI_DEFAULT_STD_SPACE", "")]), (5, 0));
    }

    #[test]
    fn priority_and_handedness() {
        let mut h = header(
            [
                [3.0, 0.0, 0.0, 1.0],
                [0.0, 3.0, 0.0, 2.0],
                [0.0, 0.0, 3.0, 3.0],
            ],
            10,
        );
        // A qform that flips x and moves the origin.
        h.qform_code = 1;
        h.quatern_b = 0.0;
        h.quatern_c = 0.0;
        h.quatern_d = 1.0; // 180° about z: x and y flipped
        h.pixdim[0] = -1.0; // and z: opposite handedness
        h.pixdim[1..4].copy_from_slice(&[3.0, 3.0, 3.0]);
        h.qoffset_x = 10.0;
        let s = AfniSettings::new();
        let (g, m) = read_geometry(&h, &s, "x.nii");
        assert_eq!((g.source, g.m[0][3]), (Source::Sform, 1.0));
        assert_eq!(
            m,
            [Message::Warning(
                "NIfTI('x.nii'): Qform/Sform handedness differ; S wins!".into()
            )]
        );
        let q = AfniSettings::from_pairs([("AFNI_NIFTI_PRIORITY", "q")]);
        let (g, m) = read_geometry(&h, &q, "x.nii");
        assert_eq!((g.source, g.m[0][3]), (Source::Qform, 10.0));
        assert!(matches!(&m[0], Message::Warning(t) if t.ends_with("Q wins!")));
        let x = AfniSettings::from_pairs([("AFNI_NIFTI_PRIORITY", "")]);
        let (g, m) = read_geometry(&h, &x, "x.nii");
        assert_eq!(g.source, Source::Sform);
        assert_eq!(
            m[0],
            Message::Warning("Illegal AFNI_NIFTI_PRIORITY=''".into())
        );
    }

    /// The quaternion of a rotation matches larmorx-io's (nibabel's) to rounding, and an
    /// identity gives AFNI's signed zeros.
    #[test]
    fn quaternions() {
        let (q, qfac) = quaternion([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
        assert_eq!(q.map(f64::to_bits), [0.0f64, 0.0, 0.0].map(f64::to_bits));
        assert_eq!(qfac, 1.0);
        let (q, qfac) = quaternion([[-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
        assert_eq!(qfac, -1.0);
        assert_eq!(q, [0.0, 1.0, 0.0]); // the third column flipped: 180° about y
        let (c, s) = (0.8, 0.6);
        let (q, _) = quaternion([[c, 0.0, -s], [0.0, 1.0, 0.0], [s, 0.0, c]]);
        assert!((q[1] + (0.1f64).sqrt()).abs() < 1e-15, "{q:?}");
    }

    /// AFNI's quaternions where only the rounding residue of the polar decomposition decides
    /// the written bits (observed by the clean-room implementer, random-testing findings
    /// `afni-automask` 20261025-00242 and -01428): a component that is 0 in exact arithmetic,
    /// and a rotation of 180° whose `a` is 0 and whose sign decides the quaternion's.
    #[test]
    fn quaternion_rounding_residue_as_afni() {
        let f = |rows: [[f32; 3]; 3]| rows.map(|r| r.map(f64::from));
        let (q, _) = quaternion(f([
            [0.934_664_25, -0.228_675_14, 0.272_232_3],
            [0.228_675_14, -0.199_637_03, -0.952_813_1],
            [0.272_232_3, 0.952_813_1, -0.134_301_28],
        ]));
        assert_eq!(
            q.map(|v| v as f32),
            [0.753_094, 4.387_546_6e-17, 0.180_742_56]
        );
        let (q, qfac) = quaternion(f([
            [-0.836_734_7, -0.979_591_85, -0.244_897_96],
            [-0.489_795_92, 0.938_775_54, 0.734_693_9],
            [-0.244_897_96, 1.469_387_8, -0.632_653_06],
        ]));
        assert_eq!(qfac, 1.0);
        assert_eq!(
            q.map(|v| v as f32),
            [0.285_714_27, -0.857_142_87, -0.428_571_43]
        );
    }

    /// AFNI-07's default on a qform in metres: the qform's matrix computed with the voxel sizes
    /// and offsets in millimetres, rounded once (the replica program's default, observed).
    #[test]
    fn qform_in_metres_converted_before_rounding() {
        let mut h = NiftiHeader::new(NiftiVersion::V1, &[2, 2, 2], DataType::F32).unwrap();
        h.qform_code = 1;
        h.sform_code = 0;
        h.xyzt_units = 1 | 8;
        (h.quatern_b, h.quatern_c, h.quatern_d) = (0.1, -0.2, 0.3);
        h.pixdim[1..4].copy_from_slice(&[0.0031, 0.0024, 0.0031]);
        (h.qoffset_x, h.qoffset_y, h.qoffset_z) = (-0.1, 0.05, 0.02);
        let (g, _) = read_geometry(&h, &AfniSettings::new(), "in.nii");
        let scaled = qform_matrix_scaled(&h, 1000.0);
        assert_eq!(g.qform_mm, Some(scaled));
        let (out, _) = output_geometry(&g, FIX);
        assert_eq!(out.srow, scaled);
        // The steps: from the file's voxel sizes, then converted.
        assert_eq!(
            out.pixdim,
            [0.0031f32, 0.0024, 0.0031].map(|p| (f64::from(p) * 1000.0) as f32)
        );
    }
}
