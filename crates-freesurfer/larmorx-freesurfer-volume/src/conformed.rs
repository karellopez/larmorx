// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/mriio.cpp`'s conformed-size checks (fs.io's queue item 3.1-28; track A relay 36), here
//! beside `getSliceDirection` (fs.volume's `coords`), which the first calls: `mriConformed`
//! (305), `MRIfindMinSize` (320) and `MRIfindRightSize` (368). VOL_GEOM arithmetic only: the
//! products of a voxel size and an extent in `float`, the rest in `double`.

use std::io::Write;

use larmorx_freesurfer_core::affine::float_to_int;
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::sse::{div64, mul, mul64};
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_mri::vol_geom::VolGeom;

use crate::coords::{MRI_CORONAL, get_slice_direction};

/// `mriConformed(vg)`: 1 when the geometry is FreeSurfer's conformed grid: coronal slices
/// (`getSliceDirection`), 256 x 256 x 256 voxels, and each voxel size `s` with `nint(s * 10000)`
/// (the product in `float`) equal to 10,000 (the sizes tested x, y, z, stopping at the first that
/// fails); 0 otherwise.
pub fn mri_conformed(g: &mut Globals, vg: &VolGeom) -> i32 {
    if get_slice_direction(vg) != MRI_CORONAL {
        return 0;
    }
    if vg.width != 256 || vg.height != 256 || vg.depth != 256 {
        return 0;
    }
    let n = |g: &mut Globals, s: f32| nint(g, f64::from(mul(s, 10000.0)));
    if n(g, vg.xsize) != 10000 || n(g, vg.ysize) != 10000 || n(g, vg.zsize) != 10000 {
        return 0;
    }
    1
}

/// The largest physical extent: `xsize * width`, `ysize * height`, `zsize * depth` (each in
/// `float`), the larger of the first two compared with the third as FreeSurfer's `if` does.
fn extents(vg: &VolGeom) -> ([f64; 3], f64) {
    let fw = f64::from(mul(vg.xsize, vg.width as f32));
    let fh = f64::from(mul(vg.ysize, vg.height as f32));
    let fd = f64::from(mul(vg.zsize, vg.depth as f32));
    let fmax = if fw > fh {
        if fw > fd { fw } else { fd }
    } else if fd > fh {
        fd
    } else {
        fh
    };
    ([fw, fh, fd], fmax)
}

/// `MRIfindMinSize(vg, &conform_width)`: the smallest voxel size (by FreeSurfer's comparisons: x
/// against y, then the smaller against z; a NaN compares false), as a `float`, and the conformed
/// width `nint(fmax / minsize * 10000) / 10000` (`int` division) of the largest physical extent
/// ([`extents`]), at least 256. The size is returned through a `double`: a signalling NaN quiet.
pub fn mri_find_min_size(g: &mut Globals, vg: &VolGeom) -> (f32, i32) {
    let (xs, ys, zs) = (vg.xsize, vg.ysize, vg.zsize);
    let min = if f64::from(xs) > f64::from(ys) {
        if f64::from(ys) > f64::from(zs) {
            zs
        } else {
            ys
        }
    } else if f64::from(zs) > f64::from(xs) {
        xs
    } else {
        zs
    };
    let (_, fmax) = extents(vg);
    let q = nint(g, mul64(div64(fmax, f64::from(min)), 10000.0)) / 10000;
    let cw = if q < 256 { 256 } else { q };
    // the size passed through a `double` (the official build keeps both conversions: a signalling
    // NaN comes back quiet), written without the round trip, which a build may fold away
    let min = if min.is_nan() {
        f32::from_bits(min.to_bits() | 0x0040_0000)
    } else {
        min
    };
    (min, cw)
}

/// `MRIfindRightSize(vg, conform_size)`: `(int) ceil(fmax / conform_size)` of the largest
/// physical extent ([`extents`]; FS-17 beyond `int`), 256 when below 256 or within 10 % above it;
/// a larger width prints FreeSurfer's warning on standard error (the three extents with two
/// decimals, the slice count).
pub fn mri_find_right_size(
    g: &mut Globals,
    err: &mut dyn Write,
    vg: &VolGeom,
    conform_size: f32,
) -> i32 {
    let ([fw, fh, fd], fmax) = extents(vg);
    let mut cw = float_to_int(
        g,
        div64(fmax, f64::from(conform_size)).ceil(),
        "MRIfindRightSize",
    );
    if cw < 256 || (f64::from(cw) - 256.0) / 256.0 < 0.1 {
        cw = 256;
    }
    if cw > 256 {
        let _ = write!(
            err,
            "WARNING ==================++++++++++++++++++++++++=======================================\n\
             The physical sizes are ({} mm, {} mm, {} mm), which cannot fit in 256^3 mm^3 volume.\n\
             The resulting volume will have {cw} slices.\n\
             If you find problems, please let us know (freesurfer@nmr.mgh.harvard.edu).\n\
             ==================================================++++++++++++++++++++++++===============\n\n",
            fmt_f(fw, 0, 2),
            fmt_f(fh, 0, 2),
            fmt_f(fd, 0, 2)
        );
    }
    cw
}
