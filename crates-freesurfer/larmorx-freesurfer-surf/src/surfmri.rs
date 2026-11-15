// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! A vertex field from a volume of one voxel per vertex: `MRIScopyMRI` (`utils/mrisurf_mri.cpp`
//! 4898; wave 3.4's fs.surf piece taken early in relay 36 of track D, for `MRISread`'s
//! group-average area).

use larmorx_freesurfer_core::cstd::double_to_int_x86;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_mri::mri::Mri;
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;

use crate::coords::Io;
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
