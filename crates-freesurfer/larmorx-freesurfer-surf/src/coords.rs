// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The coordinate operations (`fs.surfio`, queue item 3.1-49, part a): FreeSurfer 8.2.0's
//! `utils/mrisurf_metricProperties.cpp` lines 58-1006 without a clone, a transform or the metric
//! properties (`MRISsetXYZ`, `MRISexportXYZ`, `MRISimportXYZ`, `MRIScopyXYZ`,
//! `MRISsetOriginalXYZ`, `MRISsetOriginalXYZfromXYZ`, `MRISavgVetexRadius`,
//! `MRISimportVertexCoords`, `MRISreverseCoords`, `MRISreverse`, `MRISscale` (FS-113),
//! `mrisFlipPatch`, `MRIStranslate`, `MRISscaleThenTranslate` (FS-113), `MRISanisotropicScale`,
//! `MRISrotate`, `MRISmatrixMultiply`, `MRISsphericalCoords`, `MRISmoveOrigin`, `MRIScenter`,
//! `MRIScalculateCenterCOG2`, `MRIScenterCOG2`, `MRIScenterCOG`, `MRISblendXYZandTXYZ`,
//! `MRISblendXYZandNXYZ`, `MRIStranslate_along_vertex_dxdydz`), with
//! `utils/mrisurf_metricProperties_slow.h`'s `mrisComputeSurfaceDimensions` (lines 3-35) and
//! `utils/mrisurf_topology.cpp`'s `MRISreverseFaceOrder` and `mrisSetVertexFaceIndex` (lines
//! 3124-3140, 1919-1936).
//!
//! Every floating-point operation that can make a NaN, or meet two, goes through fs.core's
//! [`sse`] helpers in the official build's operand order (GCC 8.4 reorders commutative operations
//! and vectorises the bounding box's updates; the order was read from the unit oracle's object
//! code, `docs/findings/freesurfer-surfio.md` §8), so the NaNs are the official build's on every
//! platform; `sin`, `cos` of a `float` are glibc's with `reproduce` (fs.core's [`Libm`]).
//!
//! Not here yet: `MRIStalairachTransform`, `MRISrotate(src, NULL, ...)` and
//! `MRISprojectOntoTranslatedSphere` (they clone the surface, `MRISclone`, queue item 3.1-51's
//! lines; the first also reads the surface's `LTA`), `mrisDisturbVertices` (it computes the
//! metric properties).

use std::io::Write;

use larmorx_core::defects::FS_113;
use larmorx_core::math::{self, Libm};
use larmorx_freesurfer_core::diag;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_UNSUPPORTED, Fatal};
use larmorx_freesurfer_core::matrix::{self, Matrix};
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_core::utils::cope_with_logic_problem2;

use crate::dist;
use crate::mris::Mris;
use crate::topology;
use crate::views::{CANONICAL_VERTICES, CURRENT_VERTICES, ORIGINAL_VERTICES};

/// `REVERSE_X`, `REVERSE_Y`, `REVERSE_Z` (`include/mrisurf.h`).
pub const REVERSE_X: i32 = 0;
pub const REVERSE_Y: i32 = 1;
pub const REVERSE_Z: i32 = 2;

/// `utils/mrisurf_metricProperties.cpp` under the official build's root (`__FILE__`).
pub fn metric_properties_cpp() -> String {
    format!("{}/utils/mrisurf_metricProperties.cpp", diag::BUILD_ROOT)
}

fn topology_cpp() -> String {
    format!("{}/utils/mrisurf_topology.cpp", diag::BUILD_ROOT)
}

/// The places a function writes to: standard output and error, the environment it reads.
pub struct Io<'a> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
    pub env: &'a Environment,
}

fn nverts(mris: &Mris) -> usize {
    usize::try_from(mris.nvertices).unwrap_or(0)
}

/// `MRISsetXYZwkr1(mris, file, line, laterTime)`: with the surface's distances allocated
/// (`dist_alloced_flags` bit 1), the logic problem `dist not freed when setXYZ called.  Add call
/// to MRISfreeDistsButNotOrig(MRIS*)` at the caller's place (reported with
/// `FREESURFER_reportLogicErrors`); the coordinates are set anyway.
fn set_xyz_wkr1(
    g: &mut Globals,
    io: &mut Io,
    mris: &Mris,
    file: &str,
    line: i32,
) -> Result<(), Fatal> {
    if mris.dist_alloced_flags & 1 != 0 {
        cope_with_logic_problem2(
            g,
            io.env,
            io.out,
            io.err,
            None,
            "dist not freed when setXYZ called.  Add call to MRISfreeDistsButNotOrig(MRIS*)",
            file,
            line,
            "<unknown>",
        )?;
    }
    Ok(())
}

fn valid_vno(
    g: &mut Globals,
    io: &mut Io,
    mris: &Mris,
    vno: i32,
    line: i32,
) -> Result<usize, Fatal> {
    if !(0 <= vno && vno < mris.nvertices) {
        return Err(diag::assert_failed(
            g,
            io.out,
            io.err,
            &metric_properties_cpp(),
            line,
            "(0 <= vno) && (vno < mris->nvertices)",
        ));
    }
    Ok(vno as usize)
}

/// `MRISsetXYZ(mris, vno, x, y, z)` (the macro: `MRISsetXYZwkr` with the caller's `__FILE__`
/// and `__LINE__`): the vertex's current coordinates; a vertex outside the surface fails
/// `cheapAssertValidVno` (line 65, FS-03).
#[allow(clippy::too_many_arguments)]
pub fn set_xyz(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    vno: i32,
    x: f32,
    y: f32,
    z: f32,
    file: &str,
    line: i32,
) -> Result<(), Fatal> {
    set_xyz_wkr1(g, io, mris, file, line)?;
    let vno = valid_vno(g, io, mris, vno, 65)?;
    let v = mris.vertex_mut(vno);
    (v.x, v.y, v.z) = (x, y, z);
    Ok(())
}

/// The bounding box of `SURFACE_DIMENSION_CALC_*` and `mrisComputeSurfaceDimensions`: from
/// 10000 and -10000 (floats), each coordinate above the high or below the low end replacing it;
/// the centre `0.5f * (float)((double)lo + (double)hi)`.
struct Dims {
    lo: [f32; 3],
    hi: [f32; 3],
}

impl Dims {
    fn new() -> Dims {
        Dims {
            lo: [10000.0; 3],
            hi: [-10000.0; 3],
        }
    }
    fn add(&mut self, p: [f32; 3]) {
        for k in 0..3 {
            if p[k] > self.hi[k] {
                self.hi[k] = p[k];
            }
            if p[k] < self.lo[k] {
                self.lo[k] = p[k];
            }
        }
    }
    fn store(&self, mris: &mut Mris) {
        (mris.xlo, mris.ylo, mris.zlo) = (self.lo[0], self.lo[1], self.lo[2]);
        (mris.xhi, mris.yhi, mris.zhi) = (self.hi[0], self.hi[1], self.hi[2]);
        mris.xctr = centre(self.lo[0], self.hi[0]);
        mris.yctr = centre(self.lo[1], self.hi[1]);
        mris.zctr = centre(self.lo[2], self.hi[2]);
    }
}

/// `0.5f * (float)((double)lo + (double)hi)`.
fn centre(lo: f32, hi: f32) -> f32 {
    sse::mul(0.5, sse::add64(f64::from(lo), f64::from(hi)) as f32)
}

/// `mrisComputeSurfaceDimensions(mris)`: the bounding box and centre of the current coordinates
/// (every vertex, ripped too). Returns `NO_ERROR`.
pub fn compute_surface_dimensions(mris: &mut Mris) -> i32 {
    let mut d = Dims::new();
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        d.add([v.x, v.y, v.z]);
    }
    d.store(mris);
    0
}

/// `MRISexportXYZ(mris, &px, &py, &pz)`: the current coordinates as three arrays. A negative
/// vertex count is an allocation `posix_memalign` refuses: a failed `cheapAssert(0 ==
/// posix_memalign((void**)ppx, 64, n*sizeof(float)))` (line 80, FS-03).
pub fn export_xyz(
    g: &mut Globals,
    io: &mut Io,
    mris: &Mris,
) -> Result<(Vec<f32>, Vec<f32>, Vec<f32>), Fatal> {
    if mris.nvertices < 0 {
        return Err(diag::assert_failed(
            g,
            io.out,
            io.err,
            &metric_properties_cpp(),
            80,
            "0 == posix_memalign((void**)ppx, 64, n*sizeof(float))",
        ));
    }
    let n = nverts(mris);
    let (mut px, mut py, mut pz) = (
        Vec::with_capacity(n),
        Vec::with_capacity(n),
        Vec::with_capacity(n),
    );
    for vno in 0..n {
        let v = mris.vertex(vno);
        px.push(v.x);
        py.push(v.y);
        pz.push(v.z);
    }
    Ok((px, py, pz))
}

/// `MRISimportXYZ(mris, px, py, pz)`: the current coordinates from three arrays, with the
/// bounding box and centre (the logic problem of [`set_xyz`] at line 131).
pub fn import_xyz(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    px: &[f32],
    py: &[f32],
    pz: &[f32],
) -> Result<(), Fatal> {
    set_xyz_wkr1(g, io, mris, &metric_properties_cpp(), 131)?;
    let mut d = Dims::new();
    for vno in 0..nverts(mris) {
        let p = [px[vno], py[vno], pz[vno]];
        let v = mris.vertex_mut(vno);
        (v.x, v.y, v.z) = (p[0], p[1], p[2]);
        d.add(p);
    }
    d.store(mris);
    Ok(())
}

/// `MRIScopyXYZ(mris, mris_from)`: the current coordinates of another surface with as many
/// vertices (else a failed `cheapAssert(nvertices == mris_from->nvertices)`, line 154), with the
/// bounding box and centre (the logic problem of [`set_xyz`] at line 151).
pub fn copy_xyz(g: &mut Globals, io: &mut Io, mris: &mut Mris, from: &Mris) -> Result<(), Fatal> {
    set_xyz_wkr1(g, io, mris, &metric_properties_cpp(), 151)?;
    if mris.nvertices != from.nvertices {
        return Err(diag::assert_failed(
            g,
            io.out,
            io.err,
            &metric_properties_cpp(),
            154,
            "nvertices == mris_from->nvertices",
        ));
    }
    let mut d = Dims::new();
    for vno in 0..nverts(mris) {
        let f = from.vertex(vno);
        let p = [f.x, f.y, f.z];
        let v = mris.vertex_mut(vno);
        (v.x, v.y, v.z) = (p[0], p[1], p[2]);
        d.add(p);
    }
    d.store(mris);
    Ok(())
}

/// `MRISsetOriginalXYZ(mris, vno, x, y, z)` (the macro: `MRISsetOriginalXYZwkr` with the
/// caller's `__FILE__` and `__LINE__`): with the original distances allocated (bit 2), the logic
/// problem `dist_orig not freed when setOriginalXYZ called.  Add call to MRISfreeDistOrigs(MRIS*)`;
/// a vertex outside the surface fails `cheapAssertValidVno` (line 180); the vertex's original
/// coordinates. (FreeSurfer then tells the active realm trees, fs.surf's, which none of this
/// module's callers has.)
#[allow(clippy::too_many_arguments)]
pub fn set_original_xyz(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    vno: i32,
    x: f32,
    y: f32,
    z: f32,
    file: &str,
    line: i32,
) -> Result<(), Fatal> {
    if mris.dist_alloced_flags & 2 != 0 {
        cope_with_logic_problem2(
            g,
            io.env,
            io.out,
            io.err,
            None,
            "dist_orig not freed when setOriginalXYZ called.  Add call to MRISfreeDistOrigs(MRIS*)",
            file,
            line,
            "<unknown>",
        )?;
    }
    let vno = valid_vno(g, io, mris, vno, 180)?;
    let v = mris.vertex_mut(vno);
    (v.origx, v.origy, v.origz) = (x, y, z);
    Ok(())
}

/// `MRISsetOriginalXYZfromXYZ(mris)`: the original distances freed, `origxyz_status` becomes
/// `status`, every vertex's original coordinates its current ones.
pub fn set_original_xyz_from_xyz(mris: &mut Mris) {
    dist::free_dist_origs(mris);
    mris.origxyz_status = mris.status;
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        (v.origx, v.origy, v.origz) = (v.x, v.y, v.z);
    }
}

/// `MRISavgVetexRadius(mris, &stdDev)`: the mean of `sqrtf((x·x + y·y) + z·z)` (in float) over
/// every vertex, summed in double, and `sqrt(N (sum2/N - avg²) / (N - 1))` (`N` the vertex
/// count; `None` for `NULL`, not computed).
pub fn avg_vertex_radius(mris: &Mris, want_std: bool) -> (f64, Option<f64>) {
    let (mut sum, mut sum2) = (0.0f64, 0.0f64);
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        let s = sse::add(
            sse::mul(v.z, v.z),
            sse::add(sse::mul(v.x, v.x), sse::mul(v.y, v.y)),
        );
        let d = f64::from(sse::sqrt(s));
        sum = sse::add64(sum, d);
        sum2 = sse::add64(sum2, sse::mul64(d, d));
    }
    let n = f64::from(mris.nvertices);
    let avg = sse::div64(sum, n);
    let std = want_std.then(|| {
        sse::sqrt64(sse::div64(
            sse::mul64(sse::sub64(sse::div64(sum2, n), sse::mul64(avg, avg)), n),
            f64::from(mris.nvertices.wrapping_sub(1)),
        ))
    });
    (avg, std)
}

/// `MRISimportVertexCoords(mris, locations, which)`: `CURRENT_VERTICES` frees the distances and
/// sets every vertex's current coordinates from the three arrays; any other `which` is the fatal
/// error `MRISimportVertexCoords: which <n> not supported` (code -3) at the first vertex (none
/// with no vertices). Returns `NO_ERROR`.
pub fn import_vertex_coords(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    locations: [&[f32]; 3],
    which: i32,
) -> Result<i32, Fatal> {
    if which == CURRENT_VERTICES {
        dist::free_dists_but_not_orig(mris);
    }
    for vno in 0..nverts(mris) {
        if which != CURRENT_VERTICES {
            log::error_exit(
                g,
                io.err,
                ERROR_UNSUPPORTED,
                &format!("MRISimportVertexCoords: which {which} not supported"),
            )?;
            continue;
        }
        let v = mris.vertex_mut(vno);
        (v.x, v.y, v.z) = (locations[0][vno], locations[1][vno], locations[2][vno]);
    }
    Ok(0)
}

fn negate(a: &mut f32, b: &mut f32, c: &mut f32, which_direction: i32) {
    match which_direction {
        REVERSE_Y => *b = -*b,
        REVERSE_Z => *c = -*c,
        _ => *a = -*a,
    }
}

/// `MRISreverseCoords(mris, which_direction, reverse_face_order, which_coords)`: for every
/// unripped vertex, the sign of one coordinate (`REVERSE_X`, any other value too; `REVERSE_Y`;
/// `REVERSE_Z`) of the current, canonical or original set (the last through
/// [`set_original_xyz`], line 328) flips; any other set is the fatal error
/// `MRISreverseCoords: unsupported which_vertices <n>` at the first unripped vertex. With
/// `REVERSE_X` and `reverse_face_order`, the faces' order is reversed ([`reverse_face_order`]).
pub fn reverse_coords(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    which_direction: i32,
    reverse_face_order_: i32,
    which_coords: i32,
) -> Result<i32, Fatal> {
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        let (mut x, mut y, mut z) = match which_coords {
            CURRENT_VERTICES => (v.x, v.y, v.z),
            CANONICAL_VERTICES => (v.cx, v.cy, v.cz),
            ORIGINAL_VERTICES => (v.origx, v.origy, v.origz),
            _ => {
                log::error_exit(
                    g,
                    io.err,
                    ERROR_UNSUPPORTED,
                    &format!("MRISreverseCoords: unsupported which_vertices {which_coords}"),
                )?;
                unreachable!("ErrorExit with a non-zero code always ends the call")
            }
        };
        negate(&mut x, &mut y, &mut z, which_direction);
        match which_coords {
            CURRENT_VERTICES => {
                let v = mris.vertex_mut(vno);
                (v.x, v.y, v.z) = (x, y, z);
            }
            CANONICAL_VERTICES => {
                let v = mris.vertex_mut(vno);
                (v.cx, v.cy, v.cz) = (x, y, z);
            }
            _ => set_original_xyz(
                g,
                io,
                mris,
                vno as i32,
                x,
                y,
                z,
                &metric_properties_cpp(),
                328,
            )?,
        }
    }
    if which_direction == REVERSE_X && reverse_face_order_ != 0 {
        reverse_face_order(g, io, mris)?;
    }
    Ok(0)
}

/// `MRISreverse(mris, which, reverse_face_order)`: [`reverse_coords`] of the current coordinates.
pub fn reverse(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    which: i32,
    reverse_face_order_: i32,
) -> Result<i32, Fatal> {
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag != 0 {
            continue;
        }
        let (mut x, mut y, mut z) = (v.x, v.y, v.z);
        negate(&mut x, &mut y, &mut z, which);
        (v.x, v.y, v.z) = (x, y, z);
    }
    if which == REVERSE_X && reverse_face_order_ != 0 {
        reverse_face_order(g, io, mris)?;
    }
    Ok(0)
}

/// `mrisSetVertexFaceIndex(mris, vno, fno)`: the vertex's position in the face (the first match;
/// none is a failed `cheapAssert(n < VERTICES_PER_FACE)`, `mrisurf_topology.cpp` line 1929)
/// becomes its `n` entry for every listing of that face among its first `num`.
pub fn set_vertex_face_index(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    vno: usize,
    fno: usize,
) -> Result<(), Fatal> {
    let fv = mris.face(fno).v;
    let Some(n) = fv.iter().position(|&x| x == vno as i32) else {
        return Err(diag::assert_failed(
            g,
            io.out,
            io.err,
            &topology_cpp(),
            1929,
            "n < VERTICES_PER_FACE",
        ));
    };
    let vt = mris.topology_mut(vno);
    let num = usize::from(vt.num);
    if num > 0 {
        let f = vt.f.as_ref().expect("VERTEX_TOPOLOGY::f is NULL");
        let hits: Vec<usize> = (0..num).filter(|&i| f[i] == fno as i32).collect();
        let nn = vt.n.as_mut().expect("VERTEX_TOPOLOGY::n is NULL");
        for i in hits {
            nn[i] = n as u8;
        }
    }
    Ok(())
}

/// `MRISreverseFaceOrder(mris)`: every face's first and third vertices swap, and the three
/// vertices' positions in it are set again ([`set_vertex_face_index`]).
pub fn reverse_face_order(g: &mut Globals, io: &mut Io, mris: &mut Mris) -> Result<(), Fatal> {
    for fno in 0..usize::try_from(mris.nfaces).unwrap_or(0) {
        let f = mris.face_mut(fno);
        let [v0, v1, v2] = f.v;
        f.v = [v2, v1, v0];
        for vno in [v0, v1, v2] {
            set_vertex_face_index(g, io, mris, vno as usize, fno)?;
        }
    }
    let _ = topology::return_true();
    Ok(())
}

/// FS-113: after a negative scale, FreeSurfer's swap of the low and high ends writes the low end
/// back over itself, so the box stays upside down; larmorx swaps them (`reproduce` keeps the
/// original's).
fn swap_after_negative<T: Copy>(g: &mut Globals, lo: &mut T, hi: &mut T) {
    g.notices.add(
        &FS_113,
        "the bounding box's ends are swapped after a negative scale (the original leaves them reversed)",
    );
    if !g.notices.reproduce() {
        std::mem::swap(lo, hi);
    }
}

/// `MRISscale(mris, scale)`: the distances freed; every coordinate, the bounding box's ends and
/// the centre multiplied by `scale` in double (stored as float). With a negative scale the
/// box's ends are swapped (FS-113).
pub fn scale(g: &mut Globals, mris: &mut Mris, s: f64) -> i32 {
    dist::free_dists_but_not_orig(mris);
    let m = |x: f32| sse::mul64(f64::from(x), s) as f32;
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        (v.x, v.y, v.z) = (m(v.x), m(v.y), m(v.z));
    }
    (mris.xlo, mris.xhi) = (m(mris.xlo), m(mris.xhi));
    (mris.ylo, mris.yhi) = (m(mris.ylo), m(mris.yhi));
    // the official build multiplies `zhi` with the scale as the first operand
    (mris.zlo, mris.zhi) = (m(mris.zlo), sse::mul64(s, f64::from(mris.zhi)) as f32);
    if s < 0.0 {
        let (mut xl, mut xh) = (mris.xlo, mris.xhi);
        let (mut yl, mut yh) = (mris.ylo, mris.yhi);
        let (mut zl, mut zh) = (mris.zlo, mris.zhi);
        swap_after_negative(g, &mut xl, &mut xh);
        swap_after_negative(g, &mut yl, &mut yh);
        swap_after_negative(g, &mut zl, &mut zh);
        (mris.xlo, mris.xhi, mris.ylo, mris.yhi, mris.zlo, mris.zhi) = (xl, xh, yl, yh, zl, zh);
    }
    (mris.xctr, mris.yctr, mris.zctr) = (m(mris.xctr), m(mris.yctr), m(mris.zctr));
    0
}

/// `mrisFlipPatch(mris)`: every `x` negated; the box's `x` ends become `-xhi`, `-xlo`, the
/// centre's `-xctr`.
pub fn flip_patch(mris: &mut Mris) -> i32 {
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        v.x = -v.x;
    }
    let xlo = mris.xlo;
    mris.xlo = -mris.xhi;
    mris.xhi = -xlo;
    mris.xctr = -mris.xctr;
    0
}

/// `MRIStranslate(mris, dx, dy, dz)`: every vertex, the box's ends and the centre moved (float
/// additions); the distances are not freed. Where two NaNs meet, the official build's operand
/// order: the coordinate's NaN for the vertices and `zhi`, the shift's for the other eight (GCC
/// adds them as two vectors of four with the shifts as the destination: the findings §8).
pub fn translate(mris: &mut Mris, dx: f32, dy: f32, dz: f32) -> i32 {
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        v.x = sse::add(v.x, dx);
        v.y = sse::add(v.y, dy);
        v.z = sse::add(v.z, dz);
    }
    mris.xlo = sse::add(dx, mris.xlo);
    mris.xhi = sse::add(dx, mris.xhi);
    mris.ylo = sse::add(dy, mris.ylo);
    mris.yhi = sse::add(dy, mris.yhi);
    mris.zlo = sse::add(dz, mris.zlo);
    mris.zhi = sse::add(mris.zhi, dz);
    mris.xctr = sse::add(dx, mris.xctr);
    mris.yctr = sse::add(dy, mris.yctr);
    mris.zctr = sse::add(dz, mris.zctr);
    0
}

/// `MRISscaleThenTranslate(mris, sx, sy, sz, dx, dy, dz)`: the distances freed; every
/// coordinate `x·sx + dx` in double (stored as float); the box's ends and the centre scaled and
/// moved the same way in double (a negative factor swaps that axis's ends: FS-113).
#[allow(clippy::too_many_arguments)]
pub fn scale_then_translate(
    g: &mut Globals,
    mris: &mut Mris,
    sx: f64,
    sy: f64,
    sz: f64,
    dx: f64,
    dy: f64,
    dz: f64,
) {
    dist::free_dists_but_not_orig(mris);
    let st = |x: f32, s: f64, d: f64| sse::add64(sse::mul64(f64::from(x), s), d) as f32;
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        (v.x, v.y, v.z) = (st(v.x, sx, dx), st(v.y, sy, dy), st(v.z, sz, dz));
    }
    let mut lo = [
        f64::from(mris.xlo),
        f64::from(mris.ylo),
        f64::from(mris.zlo),
    ];
    let mut hi = [
        f64::from(mris.xhi),
        f64::from(mris.yhi),
        f64::from(mris.zhi),
    ];
    let mut ctr = [
        f64::from(mris.xctr),
        f64::from(mris.yctr),
        f64::from(mris.zctr),
    ];
    let s = [sx, sy, sz];
    let d = [dx, dy, dz];
    // `ylo`, `zlo`: the scale first, then the shift first; every other end and the centre: the
    // value first (the official build's vector code)
    for k in 0..3 {
        lo[k] = if k == 0 {
            sse::mul64(lo[k], s[k])
        } else {
            sse::mul64(s[k], lo[k])
        };
        hi[k] = sse::mul64(hi[k], s[k]);
    }
    for k in 0..3 {
        if s[k] < 0.0 {
            let (mut a, mut b) = (lo[k], hi[k]);
            swap_after_negative(g, &mut a, &mut b);
            (lo[k], hi[k]) = (a, b);
        }
    }
    for k in 0..3 {
        ctr[k] = sse::mul64(ctr[k], s[k]);
    }
    for k in 0..3 {
        lo[k] = if k == 0 {
            sse::add64(lo[k], d[k])
        } else {
            sse::add64(d[k], lo[k])
        };
        hi[k] = sse::add64(hi[k], d[k]);
    }
    for k in 0..3 {
        ctr[k] = sse::add64(ctr[k], d[k]);
    }
    (mris.xlo, mris.xhi) = (lo[0] as f32, hi[0] as f32);
    (mris.ylo, mris.yhi) = (lo[1] as f32, hi[1] as f32);
    (mris.zlo, mris.zhi) = (lo[2] as f32, hi[2] as f32);
    (mris.xctr, mris.yctr, mris.zctr) = (ctr[0] as f32, ctr[1] as f32, ctr[2] as f32);
}

/// `MRISanisotropicScale(mris, sx, sy, sz)`: the bounding box computed and the distances freed;
/// with any factor other than 1 every coordinate becomes `(x - x0)·sx + x0` around the box's
/// centre (in float), the box's ends likewise (put in order) and the centre recomputed from them.
/// Returns `NO_ERROR`.
pub fn anisotropic_scale(mris: &mut Mris, sx: f32, sy: f32, sz: f32) -> i32 {
    compute_surface_dimensions(mris);
    dist::free_dists_but_not_orig(mris);
    if sx == 1.0 && sy == 1.0 && sz == 1.0 {
        return 0;
    }
    let c = [mris.xctr, mris.yctr, mris.zctr];
    let s = [sx, sy, sz];
    let f = |x: f32, k: usize| sse::add(sse::mul(sse::sub(x, c[k]), s[k]), c[k]);
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        (v.x, v.y, v.z) = (f(v.x, 0), f(v.y, 1), f(v.z, 2));
    }
    // the low ends as the vertices; the high ends `x0 + sx·(xhi - x0)` (the official build's
    // operand order)
    let fh = |x: f32, k: usize| sse::add(c[k], sse::mul(s[k], sse::sub(x, c[k])));
    let lo = [f(mris.xlo, 0), f(mris.ylo, 1), f(mris.zlo, 2)];
    let hi = [fh(mris.xhi, 0), fh(mris.yhi, 1), fh(mris.zhi, 2)];
    let mut out = [(0.0f32, 0.0f32); 3];
    for k in 0..3 {
        out[k] = if lo[k] < hi[k] {
            (lo[k], hi[k])
        } else {
            (hi[k], lo[k])
        };
    }
    (mris.xlo, mris.xhi) = out[0];
    (mris.ylo, mris.yhi) = out[1];
    (mris.zlo, mris.zhi) = out[2];
    // `0.5f * (float)((double)xlo + (double)xhi)`, which GCC computes as `(xhi + xlo) * 0.5f` in
    // float (the same number: a double sum of two floats rounds to the float sum)
    mris.xctr = sse::mul(sse::add(hi[0], lo[0]), 0.5);
    mris.yctr = sse::mul(sse::add(hi[1], lo[1]), 0.5);
    mris.zctr = sse::mul(sse::add(hi[2], lo[2]), 0.5);
    0
}

/// `MRISrotate(mris, mris, alpha, beta, gamma)` (the `MRIS` view's `MRISrotate`): the distances
/// freed; every vertex rotated by the Tait-Bryan angles (radians: `alpha` about z, `beta` about
/// y, `gamma` about x), the sines and cosines of the `float` angles (glibc's with `reproduce`)
/// and every product in float, as FreeSurfer writes them.
pub fn rotate(g: &Globals, mris: &mut Mris, alpha: f32, beta: f32, gamma: f32) {
    dist::free_dists_but_not_orig(mris);
    let libm = Libm::quiet(g.mode());
    let (sa, sb, sg) = (libm.sinf(alpha), libm.sinf(beta), libm.sinf(gamma));
    let (ca, cb, cg) = (libm.cosf(alpha), libm.cosf(beta), libm.cosf(gamma));
    // the products and sums in the official build's operand order (its object code: GCC
    // reorders the commutative operations and computes `-x * cbsa + ...` as a subtraction)
    use sse::{add, mul, sub};
    let cacb = mul(ca, cb);
    let cacg = mul(ca, cg);
    let cacgsb = mul(cacg, sb);
    let sasg = mul(sg, sa);
    let cgsa = mul(cg, sa);
    let casbsg = mul(mul(sb, ca), sg);
    let cbsa = mul(cb, sa);
    let cgsasb = mul(sb, cgsa);
    let casg = mul(ca, sg);
    let sasbsg = mul(mul(sa, sb), sg);
    let cbcg = mul(cg, cb);
    let cbsg = mul(sg, cb);
    let k1 = sub(-cacgsb, sasg);
    let k2 = sub(cgsa, casbsg);
    let k3 = sub(cgsasb, casg);
    let k4 = add(cacg, sasbsg);
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        let (x, y, z) = (v.x, v.y, v.z);
        let xp = add(add(mul(k1, z), mul(cacb, x)), mul(k2, y));
        let yp = add(sub(mul(k3, z), mul(x, cbsa)), mul(k4, y));
        let zp = add(add(mul(z, cbcg), mul(x, sb)), mul(y, cbsg));
        (v.x, v.y, v.z) = (xp, yp, zp);
    }
}

/// `MRISrotate(MRIS_MP*, ...)` and `MRIStranslate_along_vertex_dxdydz(MRIS_MP*, ...)`: their
/// first step frees the `MRIS_MP`'s distances, which FreeSurfer 8.2.0 never wrote: a failed
/// assertion ([`dist::free_dists_but_not_orig_mp`]). No FreeSurfer program reaches them.
pub fn rotate_mp(g: &mut Globals, io: &mut Io) -> Fatal {
    dist::free_dists_but_not_orig_mp(g, io.out, io.err)
}

/// `MRISmatrixMultiply(mris, M)`: the distances freed; every vertex's `(x, y, z, 1)` multiplied by
/// `M` (FreeSurfer's float `MatrixMultiply`), the first three rows its new coordinates. Where
/// the product is refused (`M` not 4 x 4: its message on standard output and an error), the
/// vertex gets the zeros of the unfilled result. Returns 0.
pub fn matrix_multiply(
    g: &mut Globals,
    io: &mut Io,
    mris: &mut Mris,
    m: Option<&Matrix>,
) -> Result<i32, Fatal> {
    dist::free_dists_but_not_orig(mris);
    for vno in 0..nverts(mris) {
        let (x, y, z) = {
            let v = mris.vertex(vno);
            (v.x, v.y, v.z)
        };
        let mut xyz = Matrix::real(4, 1);
        xyz.set_relt(4, 1, 1.0);
        let mut mxyz = Matrix::real(4, 1);
        xyz.set_relt(1, 1, x);
        xyz.set_relt(2, 1, y);
        xyz.set_relt(3, 1, z);
        matrix::matrix_multiply(g, io.out, io.err, m, Some(&xyz), Some(&mut mxyz))?;
        let v = mris.vertex_mut(vno);
        (v.x, v.y, v.z) = (mxyz.relt(1, 1), mxyz.relt(2, 1), mxyz.relt(3, 1));
    }
    Ok(0)
}

/// `atan2(y, x)` as the official build gets it: correctly rounded where glibc's agrees (every
/// argument the replays measured, `docs/findings/freesurfer-libm.md`), the NaN of `x + y` when
/// either is a NaN (observed).
fn atan2(y: f64, x: f64) -> f64 {
    if x.is_nan() || y.is_nan() {
        return sse::add64(x, y);
    }
    math::atan2(y, x)
}

/// `MRISsphericalCoords(mris)`: the distances freed; every vertex's `(x, y, z)` (in double)
/// becomes `(atan2(y, x), atan2(z, sqrt(x² + y²)), sqrt((x² + y²) + z²))`, stored as float.
/// Returns 0.
pub fn spherical_coords(mris: &mut Mris) -> i32 {
    dist::free_dists_but_not_orig(mris);
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        let (x, y, z) = (f64::from(v.x), f64::from(v.y), f64::from(v.z));
        let d2 = sse::add64(sse::mul64(x, x), sse::mul64(y, y));
        let d = sse::sqrt64(d2);
        let r = sse::sqrt64(sse::add64(sse::mul64(z, z), d2));
        let theta = atan2(y, x);
        let phi = atan2(z, d);
        (v.x, v.y, v.z) = (theta as f32, phi as f32, r as f32);
    }
    0
}

/// `MRISmoveOrigin(mris, x0, y0, z0)`: the bounding box computed, the surface moved by `x0 -
/// xctr` (float), ...; then the centre set to `(x0, y0, z0)` exactly.
pub fn move_origin(mris: &mut Mris, x0: f32, y0: f32, z0: f32) {
    compute_surface_dimensions(mris);
    let (dx, dy, dz) = (
        sse::sub(x0, mris.xctr),
        sse::sub(y0, mris.yctr),
        sse::sub(z0, mris.zctr),
    );
    translate(mris, dx, dy, dz);
    (mris.xctr, mris.yctr, mris.zctr) = (x0, y0, z0);
}

/// `MRIScenter(mris, mris)`: [`move_origin`] to `(0, 0, 0)` (FreeSurfer asserts the two surfaces
/// are one).
pub fn center(mris: &mut Mris) {
    move_origin(mris, 0.0, 0.0, 0.0);
}

/// `MRIScalculateCenterCOG2(mris, ...)`: the mean of the current coordinates, summed in double
/// over every vertex and divided by the vertex count.
pub fn calculate_center_cog2(mris: &Mris) -> (f64, f64, f64) {
    let (mut x, mut y, mut z) = (0.0f64, 0.0f64, 0.0f64);
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        x = sse::add64(x, f64::from(v.x));
        y = sse::add64(y, f64::from(v.y));
        z = sse::add64(z, f64::from(v.z));
    }
    let n = f64::from(mris.nvertices);
    (sse::div64(x, n), sse::div64(y, n), sse::div64(z, n))
}

/// `MRIScenterCOG2(mris, ...)`: the surface moved by minus its mean (rounded to float,
/// [`translate`]); returns the mean.
pub fn center_cog2(mris: &mut Mris) -> (f64, f64, f64) {
    let (x, y, z) = calculate_center_cog2(mris);
    translate(mris, -x as f32, -y as f32, -z as f32);
    (x, y, z)
}

/// `MRIScenterCOG(mris)`.
pub fn center_cog(mris: &mut Mris) {
    center_cog2(mris);
}

/// `MRISblendXYZandTXYZ(mris, xyzScale, txyzScale)`: the distances freed; every vertex's current
/// coordinates `xyzScale·x + txyzScale·tx` (float).
pub fn blend_xyz_and_txyz(mris: &mut Mris, xyz_scale: f32, txyz_scale: f32) {
    dist::free_dists_but_not_orig(mris);
    let b = |x: f32, t: f32| sse::add(sse::mul(x, xyz_scale), sse::mul(t, txyz_scale));
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        (v.x, v.y, v.z) = (b(v.x, v.tx), b(v.y, v.ty), b(v.z, v.tz));
    }
}

/// `MRISblendXYZandNXYZ(mris, nxyzScale)`: the distances freed; every vertex moved by
/// `nxyzScale` times its normal (float).
pub fn blend_xyz_and_nxyz(mris: &mut Mris, nxyz_scale: f32) {
    dist::free_dists_but_not_orig(mris);
    let b = |x: f32, n: f32| sse::add(sse::mul(n, nxyz_scale), x);
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        (v.x, v.y, v.z) = (b(v.x, v.nx), b(v.y, v.ny), b(v.z, v.nz));
    }
}

/// `MRIStranslate_along_vertex_dxdydz(mris, mris, dt)` (the `MRIS` views'; its only caller
/// passes one surface twice): the distances freed; every unripped vertex moved by `dt` times its
/// step `(dx, dy, dz)` (`x + dt·dx` in double, stored as float).
pub fn translate_along_vertex_dxdydz(mris: &mut Mris, dt: f64) {
    dist::free_dists_but_not_orig(mris);
    let t = |x: f32, d: f32| sse::add64(sse::mul64(f64::from(d), dt), f64::from(x)) as f32;
    for vno in 0..nverts(mris) {
        let v = mris.vertex_mut(vno);
        if v.ripflag != 0 {
            continue;
        }
        (v.x, v.y, v.z) = (t(v.x, v.dx), t(v.y, v.dy), t(v.z, v.dz));
    }
}
