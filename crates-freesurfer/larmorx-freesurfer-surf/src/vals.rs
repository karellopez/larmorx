// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.surf's `utils/mrisurf_vals.cpp` beyond its copies ([`crate::valcopy`]; queue item 3.4-03,
//! taken early; relay 39 of track D): the hole filling of a surface map (`MRISfillHoles`, for
//! `MRISmarkEdge`'s `FillHoles`).
//!
//! Defects (`larmorx_core::defects`): FS-914 (the hole count printed, kept), FS-915 (a value at
//! the threshold or NaN in neither map).

use larmorx_core::defects::{FS_914, FS_915};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_mri::mri::Mri;
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

use crate::coords::Io;
use crate::mris::Mris;
use crate::{sclust, surfmri};

/// `MRISfillHoles(surf, fieldname, mri, thresh)`: the map (the vertices' field `fieldname`
/// copied into a new volume by `MRIcopyMRIS`, or the caller's volume of one voxel per vertex)
/// binarized (above `thresh`: 1, else 0), the vertices' `val` set to the inverted map (below
/// `thresh`: 1); the islands of the inverted map found (`sclustMapSurfClusters` at 0.5, no upper
/// threshold, positive, no minimum area: the vertices' `undefval` hold their cluster numbers,
/// largest area first), `MRISfillHoes(): found <n - 1> holes` printed, and with two or more
/// clusters every vertex of a cluster after the first set to 1 in the map (the holes filled).
/// The field's map copied back into the field (`MRIScopyMRI`), then `  MRISfillHoles() field
/// <name>, found <n - 1> holes, filled <k> vertices`; the caller's volume: `  MRISfillHoles()
/// found <n - 1> holes, filled <k> vertices`. Returns the vertices filled; -1 after `ERROR:
/// MRISfillHoles(): fieldname=<name> and mri is non-null` or `ERROR: MRISfillHoles(): both
/// fieldname and mri are null`, or when the field cannot be copied. All on standard output.
///
/// FS-915: a value equal to `thresh` or NaN is neither in the map nor in the inverted map
/// (FreeSurfer's `>` and `<`); by default the inverted map is the map's complement (noted).
/// FS-914: the count printed is -1 when no vertex is below the threshold (kept).
pub fn fill_holes(
    g: &mut Globals,
    io: &mut Io,
    surf: &mut Mris,
    fieldname: Option<&str>,
    mri: Option<&mut Mri>,
    thresh: f64,
) -> Result<i32, Fatal> {
    if let (Some(name), Some(_)) = (fieldname, &mri) {
        let _ = writeln!(
            io.out,
            "ERROR: MRISfillHoles(): fieldname={name} and mri is non-null"
        );
        return Ok(-1);
    }
    let mut fresh = None;
    let m: &mut Mri = match (mri, fieldname) {
        (Some(m), _) => m,
        (None, None) => {
            let _ = writeln!(
                io.out,
                "ERROR: MRISfillHoles(): both fieldname and mri are null"
            );
            return Ok(-1);
        }
        (None, Some(name)) => match surfmri::copy_mris(g, io, None, surf, 0, name)? {
            Ret::New(m) => fresh.insert(m),
            _ => return Ok(-1),
        },
    };
    let nv = surf.nvertices;
    for vno in 0..nv {
        let val = f64::from(mri_get_vox_val(g, io.err, m, vno, 0, 0, 0, 0)?);
        mri_set_vox_val(
            g,
            io.err,
            m,
            vno,
            0,
            0,
            0,
            f32::from(u8::from(val > thresh)),
            0,
        )?;
        let below = val < thresh;
        let inverted = if below == (val.is_nan() || val <= thresh) {
            below
        } else {
            g.notices.add(
                &FS_915,
                format!(
                    "MRISfillHoles: vertex {vno}'s value {val} is at the threshold {thresh} or NaN: in neither the map nor its holes (FreeSurfer); taken as outside the map"
                ),
            );
            if g.notices.reproduce() { below } else { true }
        };
        surf.vertex_mut(vno as usize).val = f32::from(u8::from(inverted));
    }
    let (_, nclusters) = sclust::map_surf_clusters(g, io, surf, 0.5, -1.0, 1, 0.0, None, None)?;
    if nclusters == 0 {
        g.notices.add(
            &FS_914,
            "MRISfillHoles: no vertex below the threshold: FreeSurfer prints found -1 holes (kept)",
        );
    }
    let _ = writeln!(
        io.out,
        "MRISfillHoes(): found {} holes",
        nclusters.wrapping_sub(1)
    );
    let mut nhits = 0i32;
    if nclusters > 1 {
        for vno in 0..nv {
            if surf.vertex(vno as usize).undefval > 1 {
                mri_set_vox_val(g, io.err, m, vno, 0, 0, 0, 1.0, 0)?;
                nhits = nhits.wrapping_add(1);
            }
        }
    }
    match fieldname {
        Some(name) => {
            surfmri::copy_mri(g, io, surf, m, 0, name)?;
            let _ = writeln!(
                io.out,
                "  MRISfillHoles() field {name}, found {} holes, filled {nhits} vertices",
                nclusters.wrapping_sub(1)
            );
        }
        None => {
            let _ = writeln!(
                io.out,
                "  MRISfillHoles() found {} holes, filled {nhits} vertices",
                nclusters.wrapping_sub(1)
            );
        }
    }
    Ok(nhits)
}
