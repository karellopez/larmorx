// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/mrifilter.cpp`'s derivatives (queue item 3.1-42): the Sobel filters `MRIxSobel` (1192),
//! `MRIxSobelForAllTypes` (1242), `MRIySobel` (1322), `MRIySobelForAllTypes` (1369), `MRIzSobel`
//! (1442), `MRIzSobelForAllTypes` (1490), `MRIsobel` (1125) with the gradient magnitude,
//! `MRIsobelFrame` (1109), `MRIdivergence` (923) and the six-neighbour `MRIlaplacian` (947).
//!
//! The Sobel filters smooth with `[1 2 1]` across and difference along their axis, divided by 8;
//! they leave the outer ring of their plane as it is (the x and y filters in the x-y plane of every
//! slice, the z filter in the x-z plane of every row). A uchar source is read by row pointers
//! (integer sums), another type by `MRIgetVoxVal` of frame 0 (float sums, [`sse`] for the NaNs'
//! bits). FS-554 to FS-556.

use std::io::Write;

use larmorx_core::defects::{FS_554, FS_555, FS_556};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_UNSUPPORTED, Fatal};
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::sse::{add, add64, mul, sqrt, sub};
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_UCHAR, Mri, mri_alloc, mri_alloc_sequence, mri_copy_header,
};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::arith::{Pair, mri_copy_frame};
use larmorx_freesurfer_volume::frames::mri_clone_different_type;

use crate::mem;

/// The three Sobel filters.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    fn name(self) -> &'static str {
        match self {
            Axis::X => "MRIxSobel",
            Axis::Y => "MRIySobel",
            Axis::Z => "MRIzSobel",
        }
    }
}

/// FS-555's check of a destination: its type (float, for a source that is not uchar), extent and
/// frame; by default an error (`NULL`) where they do not fit, with `reproduce` noted (FreeSurfer's
/// writes go where its row pointers lead).
fn dst_fits(
    g: &mut Globals,
    err: &mut dyn Write,
    what: &str,
    src: &Mri,
    dst: &Mri,
    frame: i32,
) -> bool {
    let (w, h, d) = (src.vg.width, src.vg.height, src.vg.depth);
    if mem::fits(dst, MRI_FLOAT, frame, w, h, d) {
        return true;
    }
    let detail = format!(
        "{what}: a destination of type {}, {}x{}x{} and {} frames for frame {frame} of a {w}x{h}x{d} source",
        dst.mtype, dst.vg.width, dst.vg.height, dst.vg.depth, dst.nframes
    );
    g.notices.add(&FS_555, detail.clone());
    if g.notices.reproduce() {
        return true;
    }
    log::error_printf(g, err, ERROR_BADPARM, &detail);
    false
}

/// Writes the float `v` at `(x, y, z)` of frame `frame` of `dst` through its row pointers.
fn put(g: &mut Globals, what: &str, dst: &mut Mri, frame: i32, x: i32, y: i32, z: i32, v: f32) {
    let sl = i64::from(frame) * i64::from(dst.vg.depth) + i64::from(z);
    mem::write(
        g,
        (&FS_555, &FS_555),
        what,
        dst,
        sl,
        i64::from(y),
        i64::from(x),
        &v.to_le_bytes(),
    );
}

/// The uchar voxel `(x, y, z)` of frame 0 (`MRIvox`), within the volume.
fn uvox(m: &Mri, x: i32, y: i32, z: i32) -> f32 {
    let (w, h) = (m.vg.width as usize, m.vg.height as usize);
    let i = (z as usize * h + y as usize) * w + x as usize;
    f32::from(m.chunk.as_ref().expect("voxels")[i])
}

/// One Sobel filter of `src` into frame `frame` of `dst` (`MRIxSobel`, `MRIySobel`, `MRIzSobel`
/// and their `ForAllTypes` forms).
fn sobel_axis(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    frame: i32,
    axis: Axis,
) -> Result<Ret<Mri>, Fatal> {
    let what = axis.name();
    let (w, h, d) = (src.vg.width, src.vg.height, src.vg.depth);
    let uchar = src.mtype == MRI_UCHAR;
    let mut fresh = None;
    let dm: &mut Mri = match dst {
        Some(m) => m,
        None => {
            let mut m = mri_alloc(g, out, err, w, h, d, MRI_FLOAT)?;
            mri_copy_header(g, out, err, src, Some(&mut m))?;
            fresh.insert(m)
        }
    };
    if uchar && dm.mtype != MRI_FLOAT {
        log::error_printf(
            g,
            err,
            ERROR_UNSUPPORTED,
            &format!(
                "{what}: unsupported src or dst type ({} or {})",
                src.mtype, dm.mtype
            ),
        );
        return Ok(Ret::Null);
    }
    if !dst_fits(g, err, what, src, dm, frame) {
        return Ok(Ret::Null);
    }
    let fast = mem::fits(dm, MRI_FLOAT, frame, w, h, d);
    // the source's frame 0 as floats (a uchar source by its row pointers; another by MRIgetVoxVal)
    let a: Vec<f32> = if uchar {
        Vec::new()
    } else {
        let mut v = Vec::with_capacity((w.max(0) * h.max(0) * d.max(0)) as usize);
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    v.push(mri_get_vox_val(g, err, src, x, y, z, 0, 0)?);
                }
            }
        }
        v
    };
    let (wu, hu) = (w as usize, h as usize);
    let val = |x: i32, y: i32, z: i32| -> f32 {
        if uchar {
            uvox(src, x, y, z)
        } else {
            a[(z as usize * hu + y as usize) * wu + x as usize]
        }
    };
    // the planes: x and y filters over (y, z) rows, z filter over (y, z) with z inside
    let (zr, yr) = match axis {
        Axis::Z => (1..d - 1, 0..h),
        _ => (0..d, 1..h - 1),
    };
    let mut writes: Vec<(i32, i32, i32, f32)> = Vec::new();
    for z in zr {
        for y in yr.clone() {
            // the three (or two) rows the filter combines at column x
            let column = |x: i32| -> (f32, f32, f32) {
                match axis {
                    Axis::X => (val(x, y - 1, z), val(x, y, z), val(x, y + 1, z)),
                    Axis::Y => (val(x, y - 1, z), 0.0, val(x, y + 1, z)),
                    Axis::Z => (val(x, y, z - 1), 0.0, val(x, y, z + 1)),
                }
            };
            // a column's combination; `role` 0, 1, 2 for the first (left), the second (middle)
            // and the loop's (right): the official build's operand orders (its object code: the
            // NaNs' bits), `2 m` as `m + m`
            let combine = |x: i32, role: u8| -> f32 {
                let (t, m, b) = column(x);
                match (axis, uchar) {
                    // (float)(*tr + 2 * *mr + *br): integer arithmetic, exact
                    (Axis::X, true) => t + 2.0 * m + b,
                    (Axis::X, false) => {
                        let m2 = add(m, m);
                        match role {
                            0 => add(add(m2, t), b),
                            1 => add(add(t, m2), b),
                            _ => add(b, add(m2, t)),
                        }
                    }
                    (_, true) => b - t,
                    (_, false) => sub(b, t),
                }
            };
            if w < 3 {
                continue;
            }
            let mut left = combine(0, 0);
            let mut middle = combine(1, 1);
            for x in 1..w - 1 {
                let right = combine(x + 1, 2);
                // x / 8 as x * 0.125 (the same value, a NaN kept)
                let v = match axis {
                    Axis::X => mul(sub(right, left), 0.125),
                    _ => mul(add(add(add(middle, middle), right), left), 0.125),
                };
                writes.push((x, y, z, v));
                left = middle;
                middle = right;
            }
        }
    }
    if fast {
        let (dw, dh) = (dm.vg.width as usize, dm.vg.height as usize);
        let base = frame as usize * dm.vox_per_vol as usize;
        let c = dm.chunk.as_mut().expect("voxels");
        for (x, y, z, v) in writes {
            let o = 4 * (base + (z as usize * dh + y as usize) * dw + x as usize);
            c[o..o + 4].copy_from_slice(&v.to_le_bytes());
        }
    } else {
        for (x, y, z, v) in writes {
            put(g, what, dm, frame, x, y, z, v);
        }
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MRIxSobel(mri_src, mri_x, frame)`: frame `frame` of `mri_x` (a float volume with the source's
/// header when none is given) the x derivative `(right - left) / 8` of `left`, `right` the
/// `[1 2 1]` sums across the rows y - 1, y, y + 1 of the columns x - 1 and x + 1, for x and y
/// within the outer ring of each slice. A uchar source with a destination that is not float:
/// `MRIxSobel: unsupported src or dst type (<src> or <dst>)` and `NULL`. A source of another type
/// reads frame 0 by `MRIgetVoxVal` (`MRIxSobelForAllTypes`). FS-555.
pub fn mri_x_sobel(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    frame: i32,
) -> Result<Ret<Mri>, Fatal> {
    sobel_axis(g, out, err, src, dst, frame, Axis::X)
}

/// `MRIySobel`: the y derivative `(right + 2 middle + left) / 8` of the differences between rows
/// y + 1 and y - 1 at the columns x - 1, x, x + 1 (as [`mri_x_sobel`] otherwise).
pub fn mri_y_sobel(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    frame: i32,
) -> Result<Ret<Mri>, Fatal> {
    sobel_axis(g, out, err, src, dst, frame, Axis::Y)
}

/// `MRIzSobel`: the z derivative of the differences between slices z + 1 and z - 1 (every row, the
/// columns and slices within the outer ring).
pub fn mri_z_sobel(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
    frame: i32,
) -> Result<Ret<Mri>, Fatal> {
    sobel_axis(g, out, err, src, dst, frame, Axis::Z)
}

/// `MRIsobel(mri_src, mri_grad, mri_mag)`: the three Sobel derivatives into frames 0, 1 and 2 of
/// `mri_grad` (a float volume of 3 frames with the source's header when none is given; a failing
/// filter's message is printed and the others run), then, with `mri_mag`, its frame 0 the
/// gradient's magnitude `sqrt(x² + y² + z²)` (`float`): through the row pointers when both are
/// float, else by `MRIgetVoxVal` and `MRIsetVoxVal`. FS-555.
pub fn mri_sobel(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    grad: Option<&mut Mri>,
    mag: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let (w, h, d) = (src.vg.width, src.vg.height, src.vg.depth);
    let mut fresh = None;
    let gm: &mut Mri = match grad {
        Some(m) => m,
        None => {
            let mut m = mri_alloc_sequence(g, out, err, w, h, d, MRI_FLOAT, 3)?;
            mri_copy_header(g, out, err, src, Some(&mut m))?;
            fresh.insert(m)
        }
    };
    mri_x_sobel(g, out, err, src, Some(gm), 0)?;
    mri_y_sobel(g, out, err, src, Some(gm), 1)?;
    mri_z_sobel(g, out, err, src, Some(gm), 2)?;
    if let Some(mm) = mag {
        if mm.mtype == MRI_FLOAT && gm.mtype == MRI_FLOAT {
            let ok = (0..3).all(|f| mem::fits(gm, MRI_FLOAT, f, w, h, d))
                && mem::fits(mm, MRI_FLOAT, 0, w, h, d);
            if !ok {
                let detail = format!(
                    "MRIsobel: a gradient of {}x{}x{} and {} frames or a magnitude of {}x{}x{} for a {w}x{h}x{d} source",
                    gm.vg.width,
                    gm.vg.height,
                    gm.vg.depth,
                    gm.nframes,
                    mm.vg.width,
                    mm.vg.height,
                    mm.vg.depth
                );
                g.notices.add(&FS_555, detail.clone());
                if !g.notices.reproduce() {
                    log::error_printf(g, err, ERROR_BADPARM, &detail);
                    return Ok(Ret::Null);
                }
            }
            for z in 0..d {
                for y in 0..h {
                    for x in 0..w {
                        let mut v = [0.0f32; 3];
                        for (f, vf) in v.iter_mut().enumerate() {
                            let sl = f as i64 * i64::from(gm.vg.depth) + i64::from(z);
                            *vf = mem::read(
                                g,
                                (&FS_555, &FS_555),
                                "MRIsobel",
                                gm,
                                sl,
                                i64::from(y),
                                i64::from(x),
                                MRI_FLOAT,
                            );
                        }
                        let m = magnitude(v);
                        put(g, "MRIsobel", mm, 0, x, y, z, m);
                    }
                }
            }
        } else {
            for z in 0..d {
                for y in 0..h {
                    for x in 0..w {
                        let mut v = [0.0f32; 3];
                        for (f, vf) in v.iter_mut().enumerate() {
                            *vf = mri_get_vox_val(g, err, gm, x, y, z, f as i32, 0)?;
                        }
                        let m = magnitude(v);
                        mri_set_vox_val(g, err, mm, x, y, z, 0, m, 0)?;
                    }
                }
            }
        }
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `sqrt(x x + y y + z z)` in `float`, as the official build adds (`z² + (x² + y²)`).
fn magnitude([x, y, z]: [f32; 3]) -> f32 {
    sqrt(add(mul(z, z), add(mul(x, x), mul(y, y))))
}

/// `MRIsobelFrame(mri_src, mri_grad, mri_mag, frame)`: [`mri_sobel`] of the source's frame `frame`
/// (copied into a one-frame volume by `MRIcopyFrame`).
pub fn mri_sobel_frame(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    grad: Option<&mut Mri>,
    mag: Option<&mut Mri>,
    frame: i32,
) -> Result<Ret<Mri>, Fatal> {
    let Ret::New(f) = mri_copy_frame(g, out, err, Pair::New(src), frame, 0)? else {
        return Ok(Ret::Null);
    };
    mri_sobel(g, out, err, &f, grad, mag)
}

/// `MRIdivergence(mri_src, mri_divergence)`: a float volume with the source's header, each voxel
/// the sum (in `double`) of [`mri_sobel`]'s three derivatives. FS-556: FreeSurfer ignores the
/// destination it is given (a new volume always); by default a given destination receives the
/// header and the sums and is returned.
pub fn mri_divergence(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let (w, h, d) = (src.vg.width, src.vg.height, src.vg.depth);
    let mut fresh = None;
    let dm: &mut Mri = match dst {
        Some(m) => {
            g.notices.add(
                &FS_556,
                if g.notices.reproduce() {
                    "MRIdivergence: the given destination left unchanged, a new volume returned, as FreeSurfer"
                } else {
                    "MRIdivergence: the given destination filled (FreeSurfer returns a new volume instead)"
                },
            );
            if g.notices.reproduce() {
                fresh.insert(mri_alloc(g, out, err, w, h, d, MRI_FLOAT)?)
            } else {
                m
            }
        }
        None => fresh.insert(mri_alloc(g, out, err, w, h, d, MRI_FLOAT)?),
    };
    mri_copy_header(g, out, err, src, Some(dm))?;
    let sobel = mri_sobel(g, out, err, src, None, None)?.unwrap_new();
    for x in 0..w {
        for y in 0..h {
            for z in 0..d {
                let dx = f64::from(mri_get_vox_val(g, err, &sobel, x, y, z, 0, 0)?);
                let dy = f64::from(mri_get_vox_val(g, err, &sobel, x, y, z, 1, 0)?);
                let dz = f64::from(mri_get_vox_val(g, err, &sobel, x, y, z, 2, 0)?);
                mri_set_vox_val(g, err, dm, x, y, z, 0, add64(add64(dx, dy), dz) as f32, 0)?;
            }
        }
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MRIlaplacian(mri_src, mri_laplacian)`: each voxel of each frame `-6` times the voxel plus its six
/// neighbours (the nearest voxel of the volume past an edge), `float` sums by `MRIgetVoxVal`;
/// without a destination, `MRIcloneDifferentType(src, MRI_FLOAT)`. FS-554: FreeSurfer takes the
/// centre voxel of every frame from frame 0 (by default the frame's own).
pub fn mri_laplacian(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    dst: Option<&mut Mri>,
) -> Result<Ret<Mri>, Fatal> {
    let (w, h, d) = (src.vg.width, src.vg.height, src.vg.depth);
    let mut fresh = None;
    let dm: &mut Mri = match dst {
        Some(m) => m,
        None => fresh.insert(mri_clone_different_type(g, out, err, src, MRI_FLOAT)?),
    };
    let near = |j: i32, n: i32| {
        if j <= 0 {
            0
        } else if j >= n {
            n - 1
        } else {
            j
        }
    };
    if src.nframes > 1 {
        g.notices.add(
            &FS_554,
            if g.notices.reproduce() {
                "MRIlaplacian: every frame's centre voxel from frame 0, as FreeSurfer"
            } else {
                "MRIlaplacian: each frame's own centre voxel (FreeSurfer takes frame 0's)"
            },
        );
    }
    let centre_frame = |f: i32| if g.notices.reproduce() { 0 } else { f };
    let cf: Vec<i32> = (0..src.nframes).map(centre_frame).collect();
    for f in 0..src.nframes {
        for x in 0..w {
            for y in 0..h {
                for z in 0..d {
                    let c = mri_get_vox_val(g, err, src, x, y, z, cf[f as usize], 0)?;
                    let mut lap = mul(c, -6.0);
                    // the official build adds the neighbours alternately into the sum and the
                    // sum into the neighbour (its object code: the NaNs' bits)
                    for (k, (xx, yy, zz)) in [
                        (near(x - 1, w), y, z),
                        (near(x + 1, w), y, z),
                        (x, near(y - 1, h), z),
                        (x, near(y + 1, h), z),
                        (x, y, near(z - 1, d)),
                        (x, y, near(z + 1, d)),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let v = mri_get_vox_val(g, err, src, xx, yy, zz, f, 0)?;
                        lap = if k % 2 == 0 { add(lap, v) } else { add(v, lap) };
                    }
                    mri_set_vox_val(g, err, dm, x, y, z, f, lap, 0)?;
                }
            }
        }
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}
