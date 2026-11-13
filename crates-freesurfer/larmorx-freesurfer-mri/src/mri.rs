// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `MRI` (`include/mri.h`, `utils/mri.cpp` 108-540): FreeSurfer's volume, its constructors
//! (`MRI::Shape`, `MRI::MRI`, `initSlices`, `initIndices`, `initGCAMorphLabel`), `MRIalloc`,
//! `MRIallocSequence`, `MRIallocHeader`, `MRIfree`, `MRIsizeof`, the intents; `MRIcopyHeader`
//! (6198), `MRIcopyPulseParameters` (13221), `MRIcopyVolGeomToMRI` and `MRIcopyVolGeomFromMRI`
//! (16256, 16279); `transform.cpp`'s `useVolGeomToMRI` and `MRIallocFromVolGeom` (365, 381).
//!
//! The voxels are one buffer of `bytes_total` bytes, as FreeSurfer's contiguous `chunk` (every
//! volume it can allocate in one piece, which is every volume on a 64-bit machine with the memory
//! for it): voxel `(c, r, s, f)` of type `T` is the `T` at element `c + r vox_per_row + s
//! vox_per_slice + f vox_per_vol`, little-endian. FreeSurfer's fallback of separate slices, taken
//! only when that allocation fails, is not kept: a buffer that cannot be allocated ends the run
//! with the fallback's message.

use std::io::Write;

use larmorx_core::defects::{FS_55, FS_56, FS_60, FS_206};
use larmorx_freesurfer_core::cname::CName;
use larmorx_freesurfer_core::diag::{DIAG_SHOW, verbose_on};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_NO_MEMORY, Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_io::xfm::{GeneralTransform, Transform, TransformData};

use crate::ctab::{ColorTable, ctab_deep_copy, ctab_free};
use crate::vol_geom::{VolGeom, copy_opt, fill_cache};

pub const MRI_UCHAR: i32 = 0;
pub const MRI_INT: i32 = 1;
pub const MRI_LONG: i32 = 2;
pub const MRI_FLOAT: i32 = 3;
pub const MRI_SHORT: i32 = 4;
pub const MRI_BITMAP: i32 = 5;
pub const MRI_TENSOR: i32 = 6;
pub const MRI_FLOAT_COMPLEX: i32 = 7;
pub const MRI_DOUBLE_COMPLEX: i32 = 8;
pub const MRI_RGB: i32 = 9;
pub const MRI_USHRT: i32 = 10;

/// `MAX_CMDS`: the command lines an `MRI` records.
pub const MAX_CMDS: usize = 1000;
/// `MAX_INDEX` (`mri.cpp`): the margin of the boundary index tables `xi`, `yi`, `zi`.
pub const MAX_INDEX: i32 = 500;
/// `MGH_VERSION`.
pub const MGH_VERSION: i32 = 1;
pub const MGZ_INTENT_UNKNOWN: i32 = -1;
pub const MGZ_INTENT_MRI: i32 = 0;
pub const MGZ_INTENT_LABEL: i32 = 1;
pub const MGZ_INTENT_SHAPE: i32 = 2;
pub const MGZ_INTENT_WARPMAP: i32 = 3;
pub const MGZ_INTENT_WARPMAP_INV: i32 = 4;
pub const MGZ_INTENT_TIME_SERIES: i32 = 5;
/// `WarpfieldDTFMT::WARPFIELD_DTFMT_UNKNOWN`.
pub const WARPFIELD_DTFMT_UNKNOWN: i32 = -1;

/// The size above which `calloc` is taken to fail (128 TiB, the x86-64 user address space; the
/// sizes FreeSurfer computes in `size_t` wrap around for negative extents and unknown types).
const CALLOC_LIMIT: u64 = 1 << 47;

/// MINC's `Transform` (`Real m[4][4]`, row by row): `larmorx_io::xfm::Transform`.
pub type Transform4 = Transform;

/// `get_linear_transform_ptr(transform)` as FreeSurfer's copy of volume_io has it
/// (`packages/minc/minc_volume_io.c`): the matrix that maps forward (the stored inverse when the
/// inverse flag is set); a transform that is not linear ends the program after
/// `get_linear_transform_ptr` on standard error (`exit(1)`; FreeSurfer reads only linear
/// transforms, so no default path meets one).
pub fn get_linear_transform_ptr(
    err: &mut dyn Write,
    t: &GeneralTransform,
) -> Result<Transform4, Fatal> {
    match &t.data {
        TransformData::Linear { forward, inverse } => {
            Ok(if t.inverse_flag { **inverse } else { **forward })
        }
        _ => {
            let _ = err.write_all(b"get_linear_transform_ptr\n");
            Err(Fatal::Exit(1))
        }
    }
}

/// `get_inverse_linear_transform_ptr(transform)`, as [`get_linear_transform_ptr`] (its message
/// `get_inverse_linear_transform_ptr`).
pub fn get_inverse_linear_transform_ptr(
    err: &mut dyn Write,
    t: &GeneralTransform,
) -> Result<Transform4, Fatal> {
    match &t.data {
        TransformData::Linear { forward, inverse } => {
            Ok(if t.inverse_flag { **forward } else { **inverse })
        }
        _ => {
            let _ = err.write_all(b"get_inverse_linear_transform_ptr\n");
            Err(Fatal::Exit(1))
        }
    }
}

/// `copy_general_transform(transform, copy)` as FreeSurfer's copy of volume_io has it: a linear
/// transform's copy takes the inverse flag into its matrices, a concatenation copies its
/// transforms (MINC 1.5.1's `copy_and_invert_transform`, `larmorx_io`'s
/// [`GeneralTransform::copy_and_invert`]); any other kind ends the program after
/// `copy_and_invert_transform bad kind` on standard error (`exit(1)`: FreeSurfer's copy supports
/// only these two).
pub fn copy_general_transform(
    err: &mut dyn Write,
    t: &GeneralTransform,
) -> Result<GeneralTransform, Fatal> {
    match &t.data {
        TransformData::Linear { .. } | TransformData::Concatenated(_) => {
            Ok(t.copy_and_invert(false))
        }
        _ => {
            let _ = err.write_all(b"copy_and_invert_transform bad kind\n");
            Err(Fatal::Exit(1))
        }
    }
}

/// `MRI_FRAME`: one frame's parameters (all 0 from `calloc`, `m_ras2vox` a 4 x 4 of zeros).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MriFrame {
    pub ftype: i32,
    pub te: f32,
    pub tr: f32,
    pub flip: f32,
    pub ti: f32,
    pub td: f32,
    pub sequence_type: i32,
    pub echo_spacing: f32,
    pub echo_train_len: f32,
    pub read_dir: [f32; 3],
    pub pe_dir: [f32; 3],
    pub slice_dir: [f32; 3],
    pub label: i32,
    /// `name[STRLEN]` (written whole by the `.mgz` writer: C's buffer semantics).
    pub name: CName,
    pub dof: i32,
    pub m_ras2vox: Option<Matrix>,
    pub thresh: f32,
    pub units: i32,
    pub dx: f64,
    pub dy: f64,
    pub dz: f64,
    pub dr: f64,
    pub dp: f64,
    pub ds: f64,
    pub bvalue: f64,
    pub tm: f64,
    pub diffusion_type: i64,
    pub d1_ramp: i64,
    pub d1_flat: i64,
    pub d1_amp: f64,
    pub d2_ramp: i64,
    pub d2_flat: i64,
    pub d2_amp: f64,
    pub d3_ramp: i64,
    pub d3_flat: i64,
    pub d3_amp: f64,
    pub d4_ramp: i64,
    pub d4_flat: i64,
    pub d4_amp: f64,
}

/// `MRI_REGION`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MriRegion {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub dx: i32,
    pub dy: i32,
    pub dz: i32,
}

/// `MRI::Shape`. Two shapes are equal when their extents and frame counts are (`operator==`
/// compares the four as a vector; `size` is not compared).
#[derive(Clone, Copy, Debug, Default)]
pub struct Shape {
    pub width: i64,
    pub height: i64,
    pub depth: i64,
    pub nframes: i64,
    pub size: i64,
}

impl PartialEq for Shape {
    fn eq(&self, o: &Shape) -> bool {
        (self.width, self.height, self.depth, self.nframes)
            == (o.width, o.height, o.depth, o.nframes)
    }
}

impl Eq for Shape {}

impl Shape {
    /// `MRI::Shape::Shape(shape)` of 3 or 4 extents: a message on standard output for another
    /// count and for each extent that is not positive (the shape is made anyway; an extent a
    /// shorter list lacks is 0 here, set by [`Mri::new_beyond`]: FS-206).
    pub fn new(out: &mut dyn Write, shape: &[i32]) -> Shape {
        let dims = shape.len();
        if dims != 3 && dims != 4 {
            let _ = writeln!(
                out,
                "MRI::Shape::Shape(): volume must be 3D or 4D (provided shape has {dims} dimensions)"
            );
        }
        for &len in shape {
            if len <= 0 {
                let _ = out.write_all(
                    b"MRI::Shape::Shape(): volume size must be greater than 0 in every dimension\n",
                );
            }
        }
        let at = |i: usize| shape.get(i).map_or(0, |&v| i64::from(v));
        let (w, h, d) = (at(0), at(1), at(2));
        let nf = if dims == 4 { at(3) } else { 1 };
        Shape {
            width: w,
            height: h,
            depth: d,
            nframes: nf,
            size: w.wrapping_mul(h).wrapping_mul(d).wrapping_mul(nf),
        }
    }
}

/// `MRI`. Its geometry is `vg` (C++'s base class); the voxels `chunk` (see the module). Fields
/// FreeSurfer's constructors leave unset are 0 here: `valid` (an `MRI`'s `valid` is never written
/// by its constructors), `mean`, `len_fname_post_fixes`.
#[derive(Debug)]
pub struct Mri {
    pub vg: VolGeom,
    pub nframes: i32,
    pub shape: Shape,
    pub imnr0: i32,
    pub imnr1: i32,
    pub xstart: f32,
    pub ystart: f32,
    pub zstart: f32,
    pub xend: f32,
    pub yend: f32,
    pub zend: f32,
    pub thick: f32,
    pub scale: i32,
    pub ps: f32,
    pub fov: f32,
    /// `xi`, `yi`, `zi`: the boundary index tables (`MAX_INDEX` entries before the first); `None`
    /// for a volume without voxels.
    pub xi: Option<Vec<i32>>,
    pub yi: Option<Vec<i32>>,
    pub zi: Option<Vec<i32>>,
    pub transform_fname: String,
    /// `transform` (MINC's `General_transform`; `None` before one is read).
    pub transform: Option<GeneralTransform>,
    /// `linear_transform`: what FreeSurfer's pointer into `transform` points at (`None`: `NULL`).
    pub linear_transform: Option<Transform4>,
    pub inverse_linear_transform: Option<Transform4>,
    pub free_transform: i32,
    pub auto_align: Option<Matrix>,
    pub outside_val: f64,
    pub mean: f64,
    pub brightness: i32,
    pub yinvert: i32,
    pub dof: i32,
    pub frames: Vec<MriFrame>,
    pub ct: Option<ColorTable>,
    pub roi: MriRegion,
    pub tr: f32,
    pub te: f32,
    pub ti: f32,
    pub flip_angle: f64,
    pub field_strength: f32,
    /// `pedir`: the phase-encoding direction (a C string: its bytes; `None` for `NULL`).
    pub pedir: Option<Vec<u8>>,
    pub orig_ras2vox: Option<Matrix>,
    pub location: f32,
    pub bvec_space: i32,
    pub bvals: Option<Matrix>,
    pub bvecs: Option<Matrix>,
    pub version: i32,
    pub intent: i32,
    pub gcamorph_image_vg: VolGeom,
    pub gcamorph_atlas_vg: VolGeom,
    pub fname_post_fixes: String,
    pub len_fname_post_fixes: i32,
    pub fname_format: String,
    pub subject_name: String,
    pub path_to_t1: String,
    pub gdf_image_stem: String,
    /// `cmdlines[0 .. ncmds]`.
    pub cmdlines: Vec<String>,
    pub warp_field_format: i32,
    pub gcamorph_spacing: i32,
    pub gcamorph_exp_k: f64,
    pub gcamorph_affine: Option<Matrix>,
    /// `gcamorphLabel[c][r][s]`, `c` slowest (`None` until `initGCAMorphLabel`).
    pub gcamorph_label: Option<Vec<i32>>,
    /// `type`.
    pub mtype: i32,
    pub ptype: i32,
    /// `MRIsizeof(type)` (`usize::MAX` for a type it does not know, FS-55).
    pub bytes_per_vox: usize,
    pub bytes_total: u64,
    pub vox_per_row: u64,
    pub vox_per_slice: u64,
    pub vox_per_vol: u64,
    pub vox_total: u64,
    pub ischunked: bool,
    pub owndata: bool,
    /// The voxels (`chunk`); `None` for a volume without them.
    pub chunk: Option<Vec<u8>>,
}

impl std::ops::Deref for Mri {
    type Target = VolGeom;
    fn deref(&self) -> &VolGeom {
        &self.vg
    }
}

impl std::ops::DerefMut for Mri {
    fn deref_mut(&mut self) -> &mut VolGeom {
        &mut self.vg
    }
}

/// `MRIsizeof(type)`: the bytes of one voxel (a complex voxel two floats; `MRI_RGB` and
/// `MRI_TENSOR` an `int` and a `float`); `size_t` -1, `usize::MAX`, for the others.
pub fn mri_sizeof(mtype: i32) -> usize {
    match mtype {
        MRI_UCHAR => 1,
        MRI_INT | MRI_RGB => 4,
        MRI_LONG => 8,
        MRI_FLOAT | MRI_TENSOR => 4,
        MRI_SHORT | MRI_USHRT => 2,
        MRI_FLOAT_COMPLEX => 8,
        _ => usize::MAX,
    }
}

/// `MRI::intentName(code)`.
pub fn intent_name(code: i32) -> &'static str {
    match code {
        MGZ_INTENT_MRI => "MRI",
        MGZ_INTENT_LABEL => "LABEL",
        MGZ_INTENT_SHAPE => "SHAPE",
        MGZ_INTENT_WARPMAP => "WARP",
        MGZ_INTENT_WARPMAP_INV => "WARP_INV",
        MGZ_INTENT_TIME_SERIES => "TIME_SERIES",
        _ => "UNKNOWN",
    }
}

/// `MRI::intentCode(name)`.
pub fn intent_code(name: &str) -> i32 {
    match name {
        "MRI" => MGZ_INTENT_MRI,
        "LABEL" => MGZ_INTENT_LABEL,
        "SHAPE" => MGZ_INTENT_SHAPE,
        "WARP" => MGZ_INTENT_WARPMAP,
        "WARP_INV" => MGZ_INTENT_WARPMAP_INV,
        "TIME_SERIES" => MGZ_INTENT_TIME_SERIES,
        _ => MGZ_INTENT_UNKNOWN,
    }
}

/// `SAMPLE_NEAREST` and the other interpolation codes (`mri.h`).
pub const SAMPLE_NEAREST: i32 = 0;
pub const SAMPLE_TRILINEAR: i32 = 1;
pub const SAMPLE_SINC: i32 = 2;
pub const SAMPLE_CUBIC: i32 = 3;
pub const SAMPLE_WEIGHTED: i32 = 4;
pub const SAMPLE_CUBIC_BSPLINE: i32 = 5;
pub const SAMPLE_VOTE: i32 = 6;

/// C's `strncasecmp(a, b, n) == 0` in the C locale (ASCII letters folded; a string ends at its
/// end or at a NUL).
fn ncase_eq(a: &[u8], b: &[u8], n: usize) -> bool {
    for i in 0..n {
        let x = a.get(i).copied().unwrap_or(0).to_ascii_lowercase();
        let y = b.get(i).copied().unwrap_or(0).to_ascii_lowercase();
        if x != y {
            return false;
        }
        if x == 0 {
            return true;
        }
    }
    true
}

/// `MRIinterpCode(name)`: the interpolation code of a method by its first three letters (any
/// case): `nea`rest, `tri`linear or `tli`, `sin`c, `cub`ic (the cubic B-spline); -1 for others.
pub fn mri_interp_code(name: &str) -> i32 {
    let s = name.as_bytes();
    if ncase_eq(s, b"nearest", 3) {
        return SAMPLE_NEAREST;
    }
    if ncase_eq(s, b"trilinear", 3) || ncase_eq(s, b"tli", 3) {
        return SAMPLE_TRILINEAR;
    }
    if ncase_eq(s, b"sinc", 3) {
        return SAMPLE_SINC;
    }
    if ncase_eq(s, b"cubic", 3) {
        return SAMPLE_CUBIC_BSPLINE;
    }
    -1
}

/// `MRIinterpString(code)`: `nearest`, `trilinear`, `sinc`, `cubic` (the B-spline code), `None`
/// (`NULL`) for the others.
pub fn mri_interp_string(code: i32) -> Option<&'static str> {
    match code {
        SAMPLE_NEAREST => Some("nearest"),
        SAMPLE_TRILINEAR => Some("trilinear"),
        SAMPLE_SINC => Some("sinc"),
        SAMPLE_CUBIC_BSPLINE => Some("cubic"),
        _ => None,
    }
}

/// `MRIprecisionCode(name)`: the voxel type named `uchar`, `short`, `ushrt`, `int`, `long` or
/// `float` (any case); -1 for others.
pub fn mri_precision_code(name: &str) -> i32 {
    let s = name.as_bytes();
    let is = |w: &[u8]| s.len() == w.len() && ncase_eq(s, w, w.len());
    if is(b"uchar") {
        MRI_UCHAR
    } else if is(b"short") {
        MRI_SHORT
    } else if is(b"ushrt") {
        MRI_USHRT
    } else if is(b"int") {
        MRI_INT
    } else if is(b"long") {
        MRI_LONG
    } else if is(b"float") {
        MRI_FLOAT
    } else {
        -1
    }
}

/// `MRIprecisionString(code)`: the name of those six types, `None` (`NULL`) for the others.
pub fn mri_precision_string(code: i32) -> Option<&'static str> {
    match code {
        MRI_UCHAR => Some("uchar"),
        MRI_SHORT => Some("short"),
        MRI_USHRT => Some("ushrt"),
        MRI_INT => Some("int"),
        MRI_LONG => Some("long"),
        MRI_FLOAT => Some("float"),
        _ => None,
    }
}

/// `ENOMEM`: what a failed `calloc` leaves in `errno` (FreeSurfer's next error message prints it).
const ENOMEM: i32 = 12;

/// `calloc(n, 1)` as glibc answers it on x86-64 Linux: `None` when it fails (`errno` `ENOMEM`).
fn calloc(g: &mut Globals, n: u64) -> Option<Vec<u8>> {
    let mut v = Vec::new();
    if n > CALLOC_LIMIT || v.try_reserve_exact(n as usize).is_err() {
        g.errno = ENOMEM;
        return None;
    }
    v.resize(n as usize, 0);
    Some(v)
}

/// A boundary index table of `len` (`calloc(len + 2 MAX_INDEX, sizeof(int))`), offset by
/// `MAX_INDEX`: index `i` in `[-MAX_INDEX, len + MAX_INDEX)` gives 0 at or below 0, `len - 1` at or
/// beyond `len`, `i` between.
fn index_table(len: i32) -> Vec<i32> {
    let n = len.wrapping_add(2 * MAX_INDEX).max(0) as usize;
    let mut t = vec![0i32; n];
    let mut i = -MAX_INDEX;
    while i < len.wrapping_add(MAX_INDEX) {
        t[(i + MAX_INDEX) as usize] = if i <= 0 {
            0
        } else if i >= len {
            len - 1
        } else {
            i
        };
        i += 1;
    }
    t
}

/// `size_t` of an `int` (C's conversion: negative values wrap around).
fn size_t(v: i32) -> u64 {
    v as i64 as u64
}

/// FS-206: the extents a shape of `len` (fewer than 3) extents lacks, set in `sh` (made by
/// [`Shape::new`] with 0 there): 1 by default; with `reproduce` what FreeSurfer reads past the
/// list (`beyond`, 0 where memory was never written). Noted when FreeSurfer's value is not 1.
fn missing_extents(g: &mut Globals, sh: &mut Shape, len: usize, beyond: &[Option<i32>]) {
    let fs: Vec<Option<i32>> = (len..3)
        .map(|i| beyond.get(i - len).copied().flatten())
        .collect();
    if fs.iter().all(|v| *v == Some(1)) {
        return;
    }
    let shown: Vec<String> = fs
        .iter()
        .map(|v| v.map_or("memory never written".to_owned(), |x| x.to_string()))
        .collect();
    let reproduce = g.notices.reproduce();
    g.notices.add(
        &FS_206,
        format!(
            "a shape of {len} extents: FreeSurfer takes the missing ones from past the list ({}){}",
            shown.join(", "),
            if reproduce {
                ", 0 where memory was never written"
            } else {
                "; taken as 1"
            }
        ),
    );
    for (k, v) in fs.iter().enumerate() {
        let x = i64::from(if reproduce { v.unwrap_or(0) } else { 1 });
        match len + k {
            0 => sh.width = x,
            1 => sh.height = x,
            _ => sh.depth = x,
        }
    }
    sh.size = sh
        .width
        .wrapping_mul(sh.height)
        .wrapping_mul(sh.depth)
        .wrapping_mul(sh.nframes);
}

impl Mri {
    /// The entry `i` (from `-MAX_INDEX`) of an index table.
    pub fn index(table: &[i32], i: i32) -> i32 {
        table[(i + MAX_INDEX) as usize]
    }

    /// `MRI::MRI(shape, dtype, alloc)`: the geometry of the shape (`xend = width / 2.0`,
    /// `xstart = -xend`, `imnr1 = depth`, `fov = width`, the region the whole volume), the frames
    /// (a 4 x 4 `m_ras2vox` of zeros each), the cached matrices of the default (coronal)
    /// geometry, and with `alloc` the voxels (zeros), `ras_good_flag = 1` and the index tables.
    /// FreeSurfer's failures: `nframes` below 0 (`MRIalloc: could not allocate <n> frame`), a
    /// buffer that cannot be allocated (`could not allocate memory for slice 1 out of <n>`, or
    /// `for <n> slices`), an index table of negative size; a type `MRIsizeof` does not know
    /// (FS-55) is refused by default (`unsupported MRI data type: <type>`), with `reproduce`
    /// allocated as FreeSurfer tries to.
    ///
    /// A shape of fewer than 3 extents (FS-206): each missing extent is 1 by default; with
    /// `reproduce`, what FreeSurfer reads past the list: [`Mri::new_beyond`]'s `beyond`, here
    /// nothing (a list made for the call ends its storage), so 0 with the notice.
    pub fn new(
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        shape: &[i32],
        dtype: i32,
        alloc: bool,
    ) -> Result<Mri, Fatal> {
        Mri::new_beyond(g, out, err, shape, &[], dtype, alloc)
    }

    /// [`Mri::new`] for a list of extents whose storage holds more places than the list has:
    /// `beyond[k]` is what the place `shape.len() + k` holds (an earlier extent the storage
    /// still has), `None` for memory never written (and every place past `beyond`). FreeSurfer
    /// reads the missing extents of a shape of fewer than 3 there (FS-206: 1 by default; with
    /// `reproduce` those values, 0 for memory never written).
    pub fn new_beyond(
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        shape: &[i32],
        beyond: &[Option<i32>],
        dtype: i32,
        alloc: bool,
    ) -> Result<Mri, Fatal> {
        let mut sh = Shape::new(out, shape);
        if shape.len() < 3 {
            missing_extents(g, &mut sh, shape.len(), beyond);
        }
        let shape = sh;
        let mut vg = VolGeom {
            width: shape.width as i32,
            height: shape.height as i32,
            depth: shape.depth as i32,
            ..VolGeom::default()
        };
        let nframes = shape.nframes as i32;
        let (w, h, d) = (vg.width, vg.height, vg.depth);
        let xend = (f64::from(w) / 2.0) as f32;
        let yend = (f64::from(h) / 2.0) as f32;
        let zend = (f64::from(d) / 2.0) as f32;
        let bytes_per_vox = mri_sizeof(dtype);
        if bytes_per_vox == usize::MAX {
            g.notices.add(
                &FS_55,
                format!(
                    "MRI type {dtype}, which MRIsizeof does not know (size_t -1 bytes a voxel)"
                ),
            );
            if !g.notices.reproduce() {
                log::fatal(g, err, 1, &format!("unsupported MRI data type: {dtype}"))?;
            }
        }
        let vox_per_row = size_t(w);
        let vox_per_slice = vox_per_row.wrapping_mul(size_t(h));
        let vox_per_vol = vox_per_slice.wrapping_mul(size_t(d));
        let vox_total = vox_per_vol.wrapping_mul(size_t(nframes));
        let bytes_total = (bytes_per_vox as u64).wrapping_mul(vox_total);
        if nframes < 0 {
            g.errno = ENOMEM;
            log::error_exit(
                g,
                err,
                ERROR_NO_MEMORY,
                &format!("MRIalloc: could not allocate {nframes} frame\n"),
            )?;
        }
        let mut frames = Vec::with_capacity(nframes.max(0) as usize);
        for _ in 0..nframes.max(0) {
            frames.push(MriFrame {
                m_ras2vox: matrix_alloc(g, out, err, 4, 4, MATRIX_REAL)?,
                ..MriFrame::default()
            });
        }
        fill_cache(g, out, err, &mut vg)?;
        let mut mri = Mri {
            vg,
            nframes,
            shape,
            imnr0: 1,
            imnr1: d,
            xstart: -xend,
            ystart: -yend,
            zstart: -zend,
            xend,
            yend,
            zend,
            thick: 1.0,
            scale: 1,
            ps: 1.0,
            fov: w as f32,
            xi: None,
            yi: None,
            zi: None,
            transform_fname: String::new(),
            transform: None,
            linear_transform: None,
            inverse_linear_transform: None,
            free_transform: 0,
            auto_align: None,
            outside_val: 0.0,
            mean: 0.0,
            brightness: 1,
            yinvert: 1,
            dof: 1,
            frames,
            ct: None,
            roi: MriRegion {
                dx: w,
                dy: h,
                dz: d,
                ..MriRegion::default()
            },
            tr: 0.0,
            te: 0.0,
            ti: 0.0,
            flip_angle: 0.0,
            field_strength: 0.0,
            pedir: None,
            orig_ras2vox: None,
            location: 0.0,
            bvec_space: 0,
            bvals: None,
            bvecs: None,
            version: MGH_VERSION,
            intent: MGZ_INTENT_MRI,
            gcamorph_image_vg: VolGeom::default(),
            gcamorph_atlas_vg: VolGeom::default(),
            fname_post_fixes: String::new(),
            len_fname_post_fixes: 0,
            fname_format: String::new(),
            subject_name: String::new(),
            path_to_t1: String::new(),
            gdf_image_stem: String::new(),
            cmdlines: Vec::new(),
            warp_field_format: WARPFIELD_DTFMT_UNKNOWN,
            gcamorph_spacing: 1,
            gcamorph_exp_k: 0.0,
            gcamorph_affine: None,
            gcamorph_label: None,
            mtype: dtype,
            ptype: 2,
            bytes_per_vox,
            bytes_total,
            vox_per_row,
            vox_per_slice,
            vox_per_vol,
            vox_total,
            ischunked: false,
            owndata: true,
            chunk: None,
        };
        if !alloc {
            return Ok(mri);
        }
        mri.vg.ras_good_flag = 1;
        mri.chunk = calloc(g, bytes_total);
        mri.ischunked = mri.chunk.is_some();
        // initSlices: the slice pointers, then (without the chunk) a buffer per slice
        let nslices = d.wrapping_mul(nframes);
        if size_t(nslices).wrapping_mul(8) > CALLOC_LIMIT {
            g.errno = ENOMEM;
            log::fatal(
                g,
                err,
                1,
                &format!("could not allocate memory for {nslices} slices"),
            )?;
        }
        if nslices > 0 && (h < 0 || !mri.ischunked) {
            // each slice's row pointers (calloc(height, 8): fails for a negative height, even
            // with the chunk) and, without the chunk, its buffer (calloc(width * height *
            // bytes_per_vox, 1)): a volume whose chunk failed cannot allocate its slices either
            g.errno = ENOMEM;
            log::fatal(
                g,
                err,
                1,
                &format!("could not allocate memory for slice 1 out of {nslices}"),
            )?;
        }
        // initIndices
        for slot in 0..3 {
            let len = [w, h, d][slot];
            let n = len.wrapping_add(2 * MAX_INDEX);
            if n < 0 {
                g.errno = ENOMEM;
                log::error_exit(
                    g,
                    err,
                    ERROR_NO_MEMORY,
                    &format!("could not allocate {n} elt index array"),
                )?;
            }
            let t = Some(index_table(len));
            match slot {
                0 => mri.xi = t,
                1 => mri.yi = t,
                _ => mri.zi = t,
            }
        }
        Ok(mri)
    }

    /// `MRI::MRI(vg, dtype, nframes, HeaderOnly)`: [`Mri::new`] with the geometry's extent, then
    /// `ras_good_flag = 1`, its voxel sizes, direction cosines, centre and file name (not its
    /// `valid` flag nor its shears), and the cached matrices recomputed.
    pub fn from_vol_geom(
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        vg: &VolGeom,
        dtype: i32,
        nframes: i32,
        header_only: bool,
    ) -> Result<Mri, Fatal> {
        let mut m = Mri::new(
            g,
            out,
            err,
            &[vg.width, vg.height, vg.depth, nframes],
            dtype,
            !header_only,
        )?;
        m.vg.ras_good_flag = 1;
        m.copy_geometry_from(vg);
        m.vg.fname.strcpy(&vg.fname);
        fill_cache(g, out, err, &mut m.vg)?;
        Ok(m)
    }

    /// The voxel sizes, direction cosines and centre of `vg`.
    fn copy_geometry_from(&mut self, vg: &VolGeom) {
        let d = &mut self.vg;
        d.xsize = vg.xsize;
        d.ysize = vg.ysize;
        d.zsize = vg.zsize;
        d.x_r = vg.x_r;
        d.x_a = vg.x_a;
        d.x_s = vg.x_s;
        d.y_r = vg.y_r;
        d.y_a = vg.y_a;
        d.y_s = vg.y_s;
        d.z_r = vg.z_r;
        d.z_a = vg.z_a;
        d.z_s = vg.z_s;
        d.c_r = vg.c_r;
        d.c_a = vg.c_a;
        d.c_s = vg.c_s;
    }

    /// `MRI::initGCAMorphLabel()`: the labels of a GCA morph, all 0.
    pub fn init_gcamorph_label(&mut self) {
        let n = i64::from(self.vg.width.max(0))
            * i64::from(self.vg.height.max(0))
            * i64::from(self.vg.depth.max(0));
        self.gcamorph_label = Some(vec![0; n as usize]);
    }
}

/// `MRIalloc(width, height, depth, type)`: a volume of one frame.
pub fn mri_alloc(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    width: i32,
    height: i32,
    depth: i32,
    mtype: i32,
) -> Result<Mri, Fatal> {
    Mri::new(g, out, err, &[width, height, depth], mtype, true)
}

/// `MRIallocSequence(width, height, depth, type, nframes)`.
#[allow(clippy::too_many_arguments)]
pub fn mri_alloc_sequence(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    width: i32,
    height: i32,
    depth: i32,
    mtype: i32,
    nframes: i32,
) -> Result<Mri, Fatal> {
    Mri::new(g, out, err, &[width, height, depth, nframes], mtype, true)
}

/// `MRIallocHeader(width, height, depth, type, nframes)`: the structure without voxels.
#[allow(clippy::too_many_arguments)]
pub fn mri_alloc_header(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    width: i32,
    height: i32,
    depth: i32,
    mtype: i32,
    nframes: i32,
) -> Result<Mri, Fatal> {
    Mri::new(g, out, err, &[width, height, depth, nframes], mtype, false)
}

/// `MRIfree(&mri)`: `NO_ERROR`, `mri` `None`; `ERROR_BADPARM` after `MRIfree: null pointer` (as an
/// error) when it is already `None`.
pub fn mri_free(g: &mut Globals, err: &mut dyn Write, mri: &mut Option<Mri>) -> i32 {
    if mri.is_none() {
        log::error_printf(g, err, ERROR_BADPARM, "MRIfree: null pointer\n");
        return ERROR_BADPARM;
    }
    *mri = None;
    NO_ERROR
}

/// `useVolGeomToMRI(src, dst)`: `ras_good_flag = 1`, the extent, voxel sizes, direction cosines,
/// centre and file name of `src`, and the cached matrices recomputed.
pub fn use_vol_geom_to_mri(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &VolGeom,
    dst: &mut VolGeom,
) -> Result<(), Fatal> {
    dst.ras_good_flag = 1;
    dst.width = src.width;
    dst.height = src.height;
    dst.depth = src.depth;
    dst.xsize = src.xsize;
    dst.ysize = src.ysize;
    dst.zsize = src.zsize;
    dst.x_r = src.x_r;
    dst.x_a = src.x_a;
    dst.x_s = src.x_s;
    dst.y_r = src.y_r;
    dst.y_a = src.y_a;
    dst.y_s = src.y_s;
    dst.z_r = src.z_r;
    dst.z_a = src.z_a;
    dst.z_s = src.z_s;
    dst.c_r = src.c_r;
    dst.c_a = src.c_a;
    dst.c_s = src.c_s;
    dst.fname.strcpy(&src.fname);
    fill_cache(g, out, err, dst)
}

/// `MRIallocFromVolGeom(vg, type, nframes, HeaderOnly)`: a volume of `vg`'s extent (without voxels
/// when `header_only`) with [`use_vol_geom_to_mri`].
pub fn mri_alloc_from_vol_geom(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vg: &VolGeom,
    mtype: i32,
    nframes: i32,
    header_only: bool,
) -> Result<Mri, Fatal> {
    let mut m = Mri::new(
        g,
        out,
        err,
        &[vg.width, vg.height, vg.depth, nframes],
        mtype,
        !header_only,
    )?;
    use_vol_geom_to_mri(g, out, err, vg, &mut m.vg)?;
    Ok(m)
}

/// `MRIcopyVolGeomToMRI(mri, vg)`: the voxel sizes, direction cosines and centre (not the extent
/// or file name; the cached matrices are not recomputed); `NO_ERROR`.
pub fn mri_copy_vol_geom_to_mri(mri: &mut Mri, vg: &VolGeom) -> i32 {
    mri.copy_geometry_from(vg);
    NO_ERROR
}

/// `MRIcopyVolGeomFromMRI(mri, vg)`: the voxel sizes, direction cosines, centre, extent and file
/// name, `valid` from `ras_good_flag`; `NO_ERROR`.
pub fn mri_copy_vol_geom_from_mri(mri: &Mri, vg: &mut VolGeom) -> i32 {
    let s = &mri.vg;
    vg.xsize = s.xsize;
    vg.ysize = s.ysize;
    vg.zsize = s.zsize;
    vg.x_r = s.x_r;
    vg.x_a = s.x_a;
    vg.x_s = s.x_s;
    vg.y_r = s.y_r;
    vg.y_a = s.y_a;
    vg.y_s = s.y_s;
    vg.z_r = s.z_r;
    vg.z_a = s.z_a;
    vg.z_s = s.z_s;
    vg.c_r = s.c_r;
    vg.c_a = s.c_a;
    vg.c_s = s.c_s;
    vg.width = s.width;
    vg.height = s.height;
    vg.depth = s.depth;
    vg.valid = s.ras_good_flag;
    vg.fname.strcpy(&s.fname);
    NO_ERROR
}

/// `MRIcopyPulseParameters(src, dst)`: flip angle, TR, TE, TI; `NO_ERROR`.
pub fn mri_copy_pulse_parameters(src: &Mri, dst: &mut Mri) -> i32 {
    dst.flip_angle = src.flip_angle;
    dst.tr = src.tr;
    dst.te = src.te;
    dst.ti = src.ti;
    NO_ERROR
}

/// `MRIcopyHeader(src, dst)`: the header of `src` into `dst` (a new volume without voxels of
/// `src`'s extent, type and frames when `None`). Copied: the version and intent, `dof`, `mean`,
/// the voxel sizes, the transform (when `src` has a linear one), its file name, `imnr0` and
/// `imnr1` (when the depths agree), `ptype`, `fov`, `thick`, `ps`, `location`, the start and end
/// coordinates, the flip angle, TR, TE and TI, the file name, direction cosines, centre and
/// `ras_good_flag`, `brightness`, `register_mat`, the names, the cached matrices, `AutoAlign` and
/// `origRas2Vox` (when `src` has them), the command lines and the colour table (when `src` has
/// one). `dst`'s own counts and sizes are recomputed from its extent and type. FS-56: FreeSurfer
/// loses `register_mat` when `dst` has none (by default it is copied); FS-60: a `dst` that owned a
/// transform keeps pointers into the freed transform when `src` has none (both modes drop them).
pub fn mri_copy_header(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let mut fresh = None;
    let d: &mut Mri = match dst {
        Some(d) => d,
        None => fresh.insert(mri_alloc_header(
            g,
            out,
            err,
            src.width,
            src.height,
            src.depth,
            src.mtype,
            src.nframes,
        )?),
    };
    d.version = src.version;
    d.intent = src.intent;
    d.dof = src.dof;
    d.mean = src.mean;
    d.vg.xsize = src.xsize;
    d.vg.ysize = src.ysize;
    d.vg.zsize = src.zsize;
    let owned = d.free_transform != 0;
    if owned {
        d.transform = None;
    }
    if src.linear_transform.is_some() {
        let (copy, fwd, inv) = match &src.transform {
            Some(t) => {
                let c = copy_general_transform(err, t)?;
                let f = get_linear_transform_ptr(err, &c)?;
                let i = get_inverse_linear_transform_ptr(err, &c)?;
                (Some(c), Some(f), Some(i))
            }
            // a linear transform without its General_transform: not made by FreeSurfer's code
            None => (None, src.linear_transform, src.inverse_linear_transform),
        };
        d.transform = copy;
        d.linear_transform = fwd;
        d.inverse_linear_transform = inv;
        d.free_transform = 1;
    } else if owned {
        g.notices.add(
            &FS_60,
            "a header copied onto a volume that owned a transform, from one without",
        );
        d.linear_transform = None;
        d.inverse_linear_transform = None;
        d.free_transform = 0;
    }
    d.transform_fname = src.transform_fname.clone();
    if d.depth == src.depth {
        d.imnr0 = src.imnr0;
        d.imnr1 = src.imnr1;
    }
    d.ptype = src.ptype;
    d.fov = src.fov;
    d.thick = src.thick;
    d.ps = src.ps;
    d.location = src.location;
    d.xstart = src.xstart;
    d.xend = src.xend;
    d.ystart = src.ystart;
    d.yend = src.yend;
    d.zstart = src.zstart;
    d.zend = src.zend;
    d.flip_angle = src.flip_angle;
    d.tr = src.tr;
    d.te = src.te;
    d.ti = src.ti;
    d.vg.fname.strcpy(&src.fname);
    {
        let (s, v) = (&src.vg, &mut d.vg);
        v.x_r = s.x_r;
        v.x_a = s.x_a;
        v.x_s = s.x_s;
        v.y_r = s.y_r;
        v.y_a = s.y_a;
        v.y_s = s.y_s;
        v.z_r = s.z_r;
        v.z_a = s.z_a;
        v.z_s = s.z_s;
        v.c_r = s.c_r;
        v.c_a = s.c_a;
        v.c_s = s.c_s;
        v.ras_good_flag = s.ras_good_flag;
    }
    d.vox_per_row = size_t(d.vg.width);
    d.vox_per_slice = d.vox_per_row.wrapping_mul(size_t(d.vg.height));
    d.vox_per_vol = d.vox_per_slice.wrapping_mul(size_t(d.vg.depth));
    d.vox_total = d.vox_per_vol.wrapping_mul(size_t(d.nframes));
    d.bytes_per_vox = mri_sizeof(d.mtype);
    d.bytes_total = (d.bytes_per_vox as u64).wrapping_mul(d.vox_total);
    d.brightness = src.brightness;
    match &src.vg.register_mat {
        Some(r) => {
            if d.vg.register_mat.is_some() {
                let old = d.vg.register_mat.take();
                d.vg.register_mat = copy_opt(g, out, err, Some(r), old)?;
            } else {
                g.notices.add(
                    &FS_56,
                    "a header copied onto a volume without a registration matrix",
                );
                if !g.notices.reproduce() {
                    d.vg.register_mat = copy_opt(g, out, err, Some(r), None)?;
                }
            }
        }
        None => d.vg.register_mat = None,
    }
    d.subject_name = src.subject_name.clone();
    d.path_to_t1 = src.path_to_t1.clone();
    d.fname_format = src.fname_format.clone();
    d.gdf_image_stem = src.gdf_image_stem.clone();
    // AffineMatrixCopy(src->i_to_r__, dst->i_to_r__): the source's 16 floats
    d.vg.i_to_r = src.vg.i_to_r;
    let old = d.vg.r_to_i.take();
    d.vg.r_to_i = copy_opt(g, out, err, src.vg.r_to_i.as_ref(), old)?;
    if let Some(a) = &src.auto_align {
        d.auto_align = copy_opt(g, out, err, Some(a), None)?;
    }
    if let Some(o) = &src.orig_ras2vox {
        d.orig_ras2vox = copy_opt(g, out, err, Some(o), None)?;
    }
    d.cmdlines = src.cmdlines.clone();
    if let Some(ct) = &src.ct {
        if g.gdiag & DIAG_SHOW != 0 && verbose_on(g) {
            let _ = out.write_all(b"MRIcopyHeader(): source has ctab. Copying ctab ...\n");
        }
        if d.ct.is_some() {
            ctab_free(g, err, &mut d.ct);
        }
        d.ct = ctab_deep_copy(g, err, Some(ct));
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}
