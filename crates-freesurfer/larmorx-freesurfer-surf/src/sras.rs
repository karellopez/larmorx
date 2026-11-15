// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! A surface's tkregister ("surface") RAS to a volume's voxels and back, with the maps and caches
//! the surface library keeps (`utils/mrisurf_mri.cpp` lines 7199-7578, part of queue item 3.4-02,
//! taken early for `mriflood.cpp`'s surface functions, queue item 3.1-43): `MRISsurfaceRASToVoxel`
//! and `MRISsurfaceRASFromVoxel` (fs.volume's `label_coords`, which track A ported for the
//! labels), `MRIS_SurfRAS2VoxelMap` with `MRIS_loadRAS2VoxelMap`, `MRIS_unloadRAS2VoxelMap`,
//! `MRIS_makeRAS2VoxelMap`, `MRIS_freeRAS2VoxelMap` and `MRIS_useRAS2VoxelMap`,
//! `MRISsurfaceRASToVoxelCached` (its map one per program: [`SurfGlobals::sras_map`]) and
//! `MRISsurfaceRASFromVoxelCached` (its matrix one per program: [`SurfGlobals::vox2sras`]).
//! `MRISsurfaceRASToVoxelCached_old` is only reached from a branch FreeSurfer compiles out
//! (`if (0)`).
//!
//! The matrix of a surface and a volume is `inv(vox2ras(mri)) RASFromSurfaceRAS_(h)` in `float`,
//! `h` a header made from the surface's volume geometry when it is valid, else the volume; a point
//! goes through it as the `float` vector `(r, a, s, 1)` (`MatrixMultiply`), its first three
//! elements returned as `double`s.
//!
//! D13 (larmorx_core::defects): FS-604 (the map's reuse across surfaces and extents), FS-605 (the
//! inverse made once).

use std::io::sink;

use larmorx_core::defects::{FS_604, FS_605};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc, matrix_multiply};
use larmorx_freesurfer_core::matrix_ops::matrix_inverse;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_mri::mri::{Mri, mri_copy_header};
use larmorx_freesurfer_mri::vol_geom::VolGeom;
use larmorx_freesurfer_volume::coords::compare_headers;
use larmorx_freesurfer_volume::label_coords;

use crate::aaa::SurfGlobals;
use crate::coords::Io;
use crate::mris::Mris;

/// The exact geometry a surface-to-voxel matrix is made from (larmorx's own key, FS-604 and
/// FS-605): the volume's extent, voxel sizes, direction cosines and centre, bit for bit, and the
/// surface's when its geometry is valid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeomKey {
    vol: [u32; 18],
    surf: Option<[u32; 18]>,
}

fn geom_bits(vg: &VolGeom) -> [u32; 18] {
    let f = [
        vg.xsize, vg.ysize, vg.zsize, vg.x_r, vg.x_a, vg.x_s, vg.y_r, vg.y_a, vg.y_s, vg.z_r,
        vg.z_a, vg.z_s, vg.c_r, vg.c_a, vg.c_s,
    ];
    let mut b = [0u32; 18];
    b[0] = vg.width as u32;
    b[1] = vg.height as u32;
    b[2] = vg.depth as u32;
    for (k, x) in f.iter().enumerate() {
        b[3 + k] = x.to_bits();
    }
    b
}

/// The key of `mris` and `mri`.
pub fn geom_key(mris: &Mris, mri: &Mri) -> GeomKey {
    GeomKey {
        vol: geom_bits(&mri.vg),
        surf: (mris.vg.valid != 0).then(|| geom_bits(&mris.vg)),
    }
}

/// Two matrices with the same size and bits (both absent counts as the same).
fn same_matrix(a: Option<&Matrix>, b: Option<&Matrix>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            a.rows == b.rows
                && a.cols == b.cols
                && a.mtype == b.mtype
                && (1..=a.rows)
                    .all(|r| (1..=a.cols).all(|c| a.relt(r, c).to_bits() == b.relt(r, c).to_bits()))
        }
        _ => false,
    }
}

/// The surface-RAS-to-voxel matrix of `mris` and `mri` (FreeSurfer's `MRISsurfaceRASToVoxel` and
/// `MRIS_loadRAS2VoxelMap` make it the same way): `None` where the product was refused.
pub fn sras2vox_matrix(
    g: &mut Globals,
    io: &mut Io,
    mris: &Mris,
    mri: &Mri,
) -> Result<Option<Matrix>, Fatal> {
    let mut lio = label_coords::Io {
        out: &mut *io.out,
        err: &mut *io.err,
        env: io.env,
    };
    label_coords::sras2vox(g, &mut lio, Some(&mris.vg), mri)
}

/// [`sras2vox_matrix`] made without a trace: a copy of the program's state and no streams. `None`
/// where FreeSurfer's computation would end the program; `Some(None)` where the product was
/// refused.
fn trial_sras2vox(
    g: &Globals,
    env: &Environment,
    mris: &Mris,
    mri: &Mri,
) -> Option<Option<Matrix>> {
    let mut gt = g.clone();
    let (mut o, mut e) = (sink(), sink());
    let mut io = Io {
        out: &mut o,
        err: &mut e,
        env,
    };
    sras2vox_matrix(&mut gt, &mut io, mris, mri).ok()
}

/// `MRISsurfaceRASToVoxel(mris, mri, r, a, s)`: a point of the surface's tkregister RAS as a voxel
/// of `mri`, the matrix made again for the call.
#[allow(clippy::too_many_arguments)]
pub fn mris_surface_ras_to_voxel(
    g: &mut Globals,
    io: &mut Io,
    mris: &Mris,
    mri: &Mri,
    r: f64,
    a: f64,
    s: f64,
) -> Result<[f64; 3], Fatal> {
    let mut lio = label_coords::Io {
        out: &mut *io.out,
        err: &mut *io.err,
        env: io.env,
    };
    label_coords::mris_surface_ras_to_voxel(g, &mut lio, Some(&mris.vg), mri, r, a, s)
}

/// `MRISsurfaceRASFromVoxel(mris, mri, x, y, z)`: a voxel of `mri` in the surface's tkregister
/// RAS (the inverse of [`mris_surface_ras_to_voxel`]'s matrix; one that cannot be inverted ends
/// the program in the product).
#[allow(clippy::too_many_arguments)]
pub fn mris_surface_ras_from_voxel(
    g: &mut Globals,
    io: &mut Io,
    mris: &Mris,
    mri: &Mri,
    x: f64,
    y: f64,
    z: f64,
) -> Result<[f64; 3], Fatal> {
    let mut lio = label_coords::Io {
        out: &mut *io.out,
        err: &mut *io.err,
        env: io.env,
    };
    label_coords::mris_surface_ras_from_voxel(g, &mut lio, Some(&mris.vg), mri, x, y, z)
}

/// `MRIS_SurfRAS2VoxelMap`: the header of the volume a matrix was made for and the matrix, with
/// the two `float` vectors a point goes through (FreeSurfer's per-thread `v1` and `v2`: one
/// thread here).
#[derive(Debug, Default)]
pub struct Ras2VoxelMap {
    /// `mri`: `MRIcopyHeader(mri, NULL)` of the volume the matrix was made for (`None`: not loaded).
    pub mri: Option<Mri>,
    /// `sras2vox` (`None`: not loaded, or the product refused).
    pub sras2vox: Option<Matrix>,
    /// The geometry the matrix was made from (larmorx's own, FS-604).
    key: Option<GeomKey>,
    v1: Option<Matrix>,
    v2: Option<Matrix>,
}

/// `MRIS_unloadRAS2VoxelMap(map)`: the header and the matrix freed (the vectors kept).
pub fn mris_unload_ras2voxel_map(map: &mut Ras2VoxelMap) {
    map.mri = None;
    map.sras2vox = None;
    map.key = None;
}

/// `MRIS_loadRAS2VoxelMap(map, mri, mris)`: nothing when the map holds a header that compares
/// equal to `mri`'s (`MRIcompareHeaders`); otherwise the map unloaded, `mri`'s header copied
/// (`MRIcopyHeader(mri, NULL)`) and the matrix made ([`sras2vox_matrix`]). FS-604: the comparison
/// ignores the volume's extent and the surface, so FreeSurfer keeps a matrix made for another
/// surface geometry or another extent (or for a header within `FLT_EPSILON`); by default the
/// matrix is made again when it would differ, noted (`reproduce`: kept).
pub fn mris_load_ras2voxel_map(
    g: &mut Globals,
    io: &mut Io,
    map: &mut Ras2VoxelMap,
    mri: &Mri,
    mris: &Mris,
) -> Result<(), Fatal> {
    if let Some(held) = map.mri.as_ref() {
        if compare_headers(Some(held), Some(mri)) == 0 {
            if g.notices.reproduce() {
                return Ok(());
            }
            let key = geom_key(mris, mri);
            if map.key == Some(key) {
                return Ok(());
            }
            let same = matches!(
                trial_sras2vox(g, io.env, mris, mri),
                Some(m) if same_matrix(m.as_ref(), map.sras2vox.as_ref())
            );
            if same {
                map.key = Some(key);
                return Ok(());
            }
            g.notices.add(
                &FS_604,
                "a surface-to-voxel map kept for a volume whose header compares equal: its matrix \
                 was made for another surface geometry or volume extent (made again here)",
            );
        }
        mris_unload_ras2voxel_map(map);
    }
    map.mri = mri_copy_header(g, io.out, io.err, mri, None)?.into_new();
    map.sras2vox = sras2vox_matrix(g, io, mris, mri)?;
    map.key = Some(geom_key(mris, mri));
    Ok(())
}

/// `MRIS_makeRAS2VoxelMap(mri, mris)`: a new map, loaded.
pub fn mris_make_ras2voxel_map(
    g: &mut Globals,
    io: &mut Io,
    mri: &Mri,
    mris: &Mris,
) -> Result<Ras2VoxelMap, Fatal> {
    let mut map = Ras2VoxelMap::default();
    mris_load_ras2voxel_map(g, io, &mut map, mri, mris)?;
    Ok(map)
}

/// `MRIS_freeRAS2VoxelMap(&map)`.
pub fn mris_free_ras2voxel_map(map: Ras2VoxelMap) {
    drop(map);
}

/// A real 4 x 1 vector with element 4 set to 1 (`VectorAlloc(4, MATRIX_REAL)`).
fn vec4(g: &mut Globals, io: &mut Io) -> Result<Matrix, Fatal> {
    let mut v = matrix_alloc(g, io.out, io.err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 vector");
    v.set_relt(4, 1, 1.0);
    Ok(v)
}

/// `m (x, y, z, 1)` through the vectors `v1` and `v2` (allocated at the first use, `v2` first):
/// the point stored as `float`s, `MatrixMultiply(m, v1, v2)` (no matrix ends the program), the
/// first three elements of `v2`.
#[allow(clippy::too_many_arguments)]
fn through(
    g: &mut Globals,
    io: &mut Io,
    m: Option<&Matrix>,
    v1: &mut Option<Matrix>,
    v2: &mut Option<Matrix>,
    x: f64,
    y: f64,
    z: f64,
) -> Result<[f64; 3], Fatal> {
    if v1.is_none() {
        *v2 = Some(vec4(g, io)?);
        *v1 = Some(vec4(g, io)?);
    }
    let (a, b) = (v1.as_mut().expect("v1"), v2.as_mut().expect("v2"));
    a.set_relt(1, 1, x as f32);
    a.set_relt(2, 1, y as f32);
    a.set_relt(3, 1, z as f32);
    matrix_multiply(g, io.out, io.err, m, Some(a), Some(b))?;
    Ok([
        f64::from(b.relt(1, 1)),
        f64::from(b.relt(2, 1)),
        f64::from(b.relt(3, 1)),
    ])
}

/// `MRIS_useRAS2VoxelMap(map, mri, r, a, s)`: the point through the map's matrix (the volume is
/// not read); a map without a matrix ends the program (`MatrixMultiply`'s message).
pub fn mris_use_ras2voxel_map(
    g: &mut Globals,
    io: &mut Io,
    map: &mut Ras2VoxelMap,
    r: f64,
    a: f64,
    s: f64,
) -> Result<[f64; 3], Fatal> {
    let Ras2VoxelMap {
        sras2vox, v1, v2, ..
    } = map;
    through(g, io, sras2vox.as_ref(), v1, v2, r, a, s)
}

/// `MRISsurfaceRASToVoxelCached(mris, mri, r, a, s)`: the program's one map
/// ([`SurfGlobals::sras_map`]) loaded for `mri` and `mris` ([`mris_load_ras2voxel_map`], FS-604)
/// and used. FreeSurfer returns `NO_ERROR`.
#[allow(clippy::too_many_arguments)]
pub fn mris_surface_ras_to_voxel_cached(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &Mris,
    mri: &Mri,
    r: f64,
    a: f64,
    s: f64,
) -> Result<[f64; 3], Fatal> {
    let map = &mut sg.sras_map;
    mris_load_ras2voxel_map(g, io, map, mri, mris)?;
    mris_use_ras2voxel_map(g, io, map, r, a, s)
}

/// `MRISsurfaceRASFromVoxelCached(mris, mri, x, y, z)`: a voxel of `mri` in the surface's
/// tkregister RAS through the program's one inverse matrix ([`SurfGlobals::vox2sras`]), made at
/// the first call (the inverse of [`sras2vox_matrix`]; when it cannot be inverted it is made again
/// at the next call, and the product ends the program). FS-605: FreeSurfer keeps the first call's
/// matrix for every later surface and volume; by default it is made again when the geometry
/// changes and it would differ, noted (`reproduce`: kept). FreeSurfer returns `NO_ERROR`.
#[allow(clippy::too_many_arguments)]
pub fn mris_surface_ras_from_voxel_cached(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &Mris,
    mri: &Mri,
    x: f64,
    y: f64,
    z: f64,
) -> Result<[f64; 3], Fatal> {
    if sg.vox2sras.m.is_some() && !g.notices.reproduce() {
        let key = geom_key(mris, mri);
        if sg.vox2sras.key != Some(key) {
            let fresh = trial_sras2vox(g, io.env, mris, mri).map(|m| {
                let mut gt = g.clone();
                let (mut o, mut e) = (sink(), sink());
                matrix_inverse(&mut gt, &mut o, &mut e, m.as_ref(), None)
                    .ok()
                    .and_then(|r| r.into_new())
            });
            match fresh {
                Some(Some(inv)) if same_matrix(Some(&inv), sg.vox2sras.m.as_ref()) => {
                    sg.vox2sras.key = Some(key);
                }
                _ => {
                    g.notices.add(
                        &FS_605,
                        "the voxel-to-surface conversion's matrix kept from its first call for \
                         another surface or volume (made again here)",
                    );
                    sg.vox2sras.m = None;
                }
            }
        }
    }
    if sg.vox2sras.m.is_none() {
        let m = sras2vox_matrix(g, io, mris, mri)?;
        sg.vox2sras.m = matrix_inverse(g, io.out, io.err, m.as_ref(), None)?.into_new();
        sg.vox2sras.key = Some(geom_key(mris, mri));
        // FreeSurfer allocates the two vectors each time it makes the matrix
        sg.vox2sras.v1 = None;
    }
    let c = &mut sg.vox2sras;
    through(g, io, c.m.as_ref(), &mut c.v1, &mut c.v2, x, y, z)
}

/// `MRISsurfaceRASFromVoxelCached`'s statics: `m_vox2sras`, `v1`, `v2` (one thread), with the
/// geometry the matrix was made from (larmorx's own, FS-605).
#[derive(Clone, Debug, Default)]
pub struct Vox2SrasCache {
    pub m: Option<Matrix>,
    key: Option<GeomKey>,
    v1: Option<Matrix>,
    v2: Option<Matrix>,
}
