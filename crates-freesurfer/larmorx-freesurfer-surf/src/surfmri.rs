// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! A vertex field from a volume of one voxel per vertex and back: `MRIScopyMRI`
//! (`utils/mrisurf_mri.cpp` 4898; wave 3.4's fs.surf piece taken early in relay 36 of track D, for
//! `MRISread`'s group-average area) and `MRIcopyMRIS` (5084; relay 37, for `MRISwriteValues`'
//! volume formats); `MRIScomputeDistanceToSurface` (7172; queue item 3.4-02, taken early in relay
//! 44: mri_relabel_hypointensities', mri_ca_label's, mri_normalize's and mri_aparc2aseg's signed
//! distance to a surface).

use larmorx_freesurfer_core::cstd::double_to_int_x86;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, Mri, mri_alloc_sequence};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

use larmorx_freesurfer_core::sse::div;
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_filter::fastmarch::{DTRANS_MODE_SIGNED, mri_distance_transform};
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_volume::arith::mri_clone;
use larmorx_freesurfer_volume::resize::mri_extract_region_and_pad;

use crate::aaa::SurfGlobals;
use crate::coords::Io;
use crate::mriflood::mris_fill_interior;
use crate::mris::{Mris, Vertex};

/// C's conversion of a `float` to an integer field (`cvttss2si`, then the field's low bits).
fn to_int(v: f32) -> i32 {
    double_to_int_x86(f64::from(v))
}

/// The field `name` of a vertex set to `val`, as `MRIScopyMRI`'s `strcmp` chain sets it; `false`
/// for a name it does not know.
fn set_field(v: &mut Vertex, name: &[u8], val: f32) -> bool {
    match name {
        b"val" => v.val = val,
        b"curv" => v.curv = val,
        b"border" => v.border = to_int(val) as i8,
        b"stat" => v.stat = val,
        b"valbak" => v.valbak = val,
        b"val2" => v.val2 = val,
        b"val2bak" => v.val2bak = val,
        b"imag_val" => v.imag_val = val,
        b"curvbak" => v.curvbak = val,
        b"fsmask" => v.fsmask = val,
        b"nc" => v.nc = val,
        b"undefval" => v.undefval = to_int(val),
        b"marked" => v.marked = to_int(val) as i16,
        b"marked2" => v.marked2 = to_int(val) as i16,
        b"marked3" => v.marked3 = to_int(val) as i16,
        b"x" => v.x = val,
        b"y" => v.y = val,
        b"z" => v.z = val,
        b"annotation" => v.annotation = to_int(val),
        b"ripflag" => v.ripflag = to_int(val) as i8,
        b"area" => v.area = val,
        b"group_avg_area" => v.group_avg_area = val,
        b"K" => v.K = val,
        b"H" => v.H = val,
        b"k1" => v.k1 = val,
        b"k2" => v.k2 = val,
        b"nx" => v.nx = val,
        b"ny" => v.ny = val,
        b"nz" => v.nz = val,
        b"tx" => v.tx = val,
        b"ty" => v.ty = val,
        b"tz" => v.tz = val,
        b"dx" => v.dx = val,
        b"dy" => v.dy = val,
        b"dz" => v.dz = val,
        b"tdx" => v.tdx = val,
        b"tdy" => v.tdy = val,
        b"tdz" => v.tdz = val,
        _ => return false,
    }
    true
}

/// `MRIScopyMRI(surf, src, frame, field)`: the volume's voxels of `frame`, in slice, row, column
/// order, into the vertices' `field` in turn (a `float` converted as C converts it for the
/// integer fields: `border`, `undefval`, `marked`, `marked2`, `marked3`, `annotation`,
/// `ripflag`). With `Gdiag_no` above 0: `MRIScopyMRI` on standard output first. Returns 1 after
/// `ERROR: MRIScopyMRI: Surf/Src dimension mismatch.` when the voxel count (width x height x
/// depth, an `int` product) is not the vertex count, after `ERROR: MRIScopyMRI: requested frame
/// number is too large.` and `ERROR:   requested = <f>, max = <n>` for a frame not below the frame
/// count, or after `ERROR: MRIScopyMRI(): Field <name> not supported` at the first voxel for a
/// field it does not know; else 0.
pub fn copy_mri(
    g: &mut Globals,
    io: &mut Io,
    surf: &mut Mris,
    src: &Mri,
    frame: i32,
    field: &str,
) -> Result<i32, Fatal> {
    if g.gdiag_no > 0 {
        let _ = writeln!(io.out, "MRIScopyMRI");
    }
    let nvox = src.width.wrapping_mul(src.height).wrapping_mul(src.depth);
    if surf.nvertices != nvox {
        let _ = writeln!(io.out, "ERROR: MRIScopyMRI: Surf/Src dimension mismatch.");
        return Ok(1);
    }
    if frame >= src.nframes {
        let _ = writeln!(
            io.out,
            "ERROR: MRIScopyMRI: requested frame number is too large."
        );
        let _ = writeln!(
            io.out,
            "ERROR:   requested = {frame}, max = {}",
            src.nframes
        );
        return Ok(1);
    }
    let name = field.as_bytes();
    let name = &name[..name.iter().position(|&c| c == 0).unwrap_or(name.len())];
    let mut vtx = 0usize;
    for s in 0..src.depth {
        for r in 0..src.height {
            for c in 0..src.width {
                let val = mri_get_vox_val(g, io.err, src, c, r, s, frame, 0)?;
                if !set_field(surf.vertex_mut(vtx), name, val) {
                    let _ = writeln!(
                        io.out,
                        "ERROR: MRIScopyMRI(): Field {} not supported",
                        String::from_utf8_lossy(name)
                    );
                    return Ok(1);
                }
                vtx += 1;
            }
        }
    }
    Ok(0)
}

/// The field `name` of a vertex as `MRIcopyMRIS`'s `strcmp` chain reads it (an integer field
/// converted to `float` as C converts it, to nearest); `None` for a name it does not know.
fn get_field(v: &Vertex, vnum: i16, name: &[u8]) -> Option<f32> {
    Some(match name {
        b"val" => v.val,
        b"curv" => v.curv,
        b"border" => f32::from(v.border),
        b"marked" => f32::from(v.marked),
        b"marked2" => f32::from(v.marked2),
        b"stat" => v.stat,
        b"d" => v.d,
        b"valbak" => v.valbak,
        b"val2" => v.val2,
        b"val2bak" => v.val2bak,
        b"imag_val" => v.imag_val,
        b"curvbak" => v.curvbak,
        b"fsmask" => v.fsmask,
        b"fieldsign" => v.fieldsign,
        b"nc" => v.nc,
        b"undefval" => v.undefval as f32,
        b"marked3" => f32::from(v.marked3),
        b"x" => v.x,
        b"y" => v.y,
        b"z" => v.z,
        b"vnum" => f32::from(vnum),
        b"ripflag" => f32::from(v.ripflag),
        b"annotation" => v.annotation as f32,
        b"area" => v.area,
        b"group_avg_area" => v.group_avg_area,
        b"K" => v.K,
        b"H" => v.H,
        b"k1" => v.k1,
        b"k2" => v.k2,
        b"nx" => v.nx,
        b"ny" => v.ny,
        b"nz" => v.nz,
        b"tx" => v.tx,
        b"ty" => v.ty,
        b"tz" => v.tz,
        b"tdx" => v.tdx,
        b"tdy" => v.tdy,
        b"tdz" => v.tdz,
        b"dx" => v.dx,
        b"dy" => v.dy,
        b"dz" => v.dz,
        b"mean" => v.mean,
        _ => return None,
    })
}

/// `MRIcopyMRIS(mri, surf, frame, field)`: the vertices' `field` in turn (`val`, `curv`, `border`,
/// the marks, `stat`, `d`, the value backups, `imag_val`, `curvbak`, `fsmask`, `fieldsign`, `nc`,
/// `undefval`, `x`, `y`, `z`, the neighbour count `vnum`, `ripflag`, `annotation`, the areas, `K`,
/// `H`, `k1`, `k2`, the normals, the temporary positions and moves, the moves, `mean`; integer
/// fields converted to `float`) into frame `frame` of the volume's voxels in slice, row, column
/// order (`MRIsetVoxVal`), into a new float volume of one voxel per vertex and `frame + 1` frames
/// when there is none. `Null` after `ERROR: MRIcopyMRIS: Surf/Src dimension mismatch.` when the
/// voxel count (width x height x depth, an `int` product) is not the vertex count, after `ERROR:
/// MRIScopyMRI: requested frame number is too large.` and `ERROR:   requested = <f>, max = <n>`
/// (FreeSurfer's own wording) for a frame not below the frame count, or after `ERROR:
/// MRIcopyMRIS(): field <name> not supported` at the first voxel for a field it does not know (all
/// on standard output; a given volume keeps the voxels set before).
pub fn copy_mris(
    g: &mut Globals,
    io: &mut Io,
    mri: Option<&mut Mri>,
    surf: &Mris,
    frame: i32,
    field: &str,
) -> Result<Ret<Mri>, Fatal> {
    let mut fresh = None;
    let m: &mut Mri = match mri {
        Some(m) => m,
        None => fresh.insert(mri_alloc_sequence(
            g,
            io.out,
            io.err,
            surf.nvertices,
            1,
            1,
            MRI_FLOAT,
            frame.wrapping_add(1),
        )?),
    };
    let nvox = m.width.wrapping_mul(m.height).wrapping_mul(m.depth);
    if surf.nvertices != nvox {
        let _ = writeln!(io.out, "ERROR: MRIcopyMRIS: Surf/Src dimension mismatch.");
        return Ok(Ret::Null);
    }
    if frame >= m.nframes {
        let _ = writeln!(
            io.out,
            "ERROR: MRIScopyMRI: requested frame number is too large."
        );
        let _ = writeln!(io.out, "ERROR:   requested = {frame}, max = {}", m.nframes);
        return Ok(Ret::Null);
    }
    let name = field.as_bytes();
    let name = &name[..name.iter().position(|&c| c == 0).unwrap_or(name.len())];
    let mut vtx = 0usize;
    let (w, h, d) = (m.width, m.height, m.depth);
    for s in 0..d {
        for r in 0..h {
            for c in 0..w {
                let Some(val) = get_field(surf.vertex(vtx), surf.topology(vtx).vnum, name) else {
                    let _ = writeln!(
                        io.out,
                        "ERROR: MRIcopyMRIS(): field {} not supported",
                        String::from_utf8_lossy(name)
                    );
                    return Ok(Ret::Null);
                };
                mri_set_vox_val(g, io.err, m, c, r, s, frame, val, 0)?;
                vtx += 1;
            }
        }
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `PAD`: the margin of the interior's volume (voxels) and the distances' limit (mm).
const PAD: i32 = 10;

/// `MRIScomputeDistanceToSurface(mris, mri_dist, resolution)`: the surface's interior
/// (`MRISfillInterior` at `resolution`) into a clone of `mri_dist` (its extent and header, voxels
/// zero) or, without one, into a volume of the surface's bounding box; without `mri_dist` that
/// volume cut to its whole extent with `PAD` (10) voxels of margin (`MRIextractRegionAndPad`, its
/// messages), else copied (`MRIcopy`); then the signed distance to the interior's edge
/// (`MRIdistanceTransform` of label 1, negative inside, up to `nint(10 / xsize)` voxels: the
/// quotient in `float`) into `mri_dist` or a new volume of the mask's header.
pub fn compute_distance_to_surface(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
    mri_dist: Option<&mut Mri>,
    resolution: f32,
) -> Result<Ret<Mri>, Fatal> {
    let res = f64::from(resolution);
    let (mask, mri_dist) = match mri_dist {
        Some(d) => {
            let mut tmp = mri_clone(g, io.out, io.err, d, None)?.unwrap_new();
            mris_fill_interior(g, sg, io, mris, res, Some(&mut tmp))?;
            let mask = mri_copy(g, io.out, io.err, &tmp, None)?.unwrap_new();
            (mask, Some(d))
        }
        None => {
            let mut tmp = mris_fill_interior(g, sg, io, mris, res, None)?.unwrap_new();
            let mask = mri_extract_region_and_pad(g, io.out, io.err, &mut tmp, None, PAD)?;
            (mask, None)
        }
    };
    let max_dist = nint(g, f64::from(div(PAD as f32, mask.vg.xsize))) as f32;
    mri_distance_transform(
        g,
        io.out,
        io.err,
        io.env,
        &mask,
        mri_dist,
        1,
        max_dist,
        DTRANS_MODE_SIGNED,
        None,
    )
}
