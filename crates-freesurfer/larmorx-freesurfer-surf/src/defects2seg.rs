// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! A surface's topological defects painted into a volume: FreeSurfer 8.2.0's `MRISdefects2Seg`
//! (`utils/mrisurf_defect.cpp` lines 15431-15531), the one function of fs.defect that a wave-3.2
//! program reaches (`mri_label2vol --defects`, which `defect2seg` runs in both of recon-all's
//! streams). Taken early into fs.surf by track A's relay 56 (the orchestrator's decision of
//! 2026-11-23): it needs only the surface, its face normals and the surface-to-voxel map.
//!
//! For every vertex whose defect number (the `float` of the overlay's voxel `n`, converted to
//! `int` as C converts it: FS-17) is not 0, every face around it: the centroid of its three
//! corners in `double` (sums of the `float` coordinates divided by 3), its normal at its first
//! corner (`mrisNormalFace(surf, face, 0, norm)`, the official build's own copy:
//! [`crate::mp_slow::normal_face`]), and the two points a fifth of the volume's column size
//! below and above the face (`centroid + (sign * delta) * normal` in `double`, sign -1 then +1)
//! taken to voxels by the surface-to-voxel map of the volume ([`sras::mris_make_ras2voxel_map`],
//! made before the size check), rounded with `nint`; each voxel inside the volume gets
//! `defect + offset` (frame 0, `MRIsetVoxVal`).
//!
//! The colour table: `CTABalloc(max(defectnomax + offset + 2, the volume's table's entries))`
//! (random colours: FreeSurfer's `randomNumber`), `CTABunique(ct, 10)`, the entries `offset` to
//! `defectnomax + offset` named `Defect-%03d`, the volume's own table merged over it
//! (`CTABmerge`, which keeps the volume's entries where both have one) and the result made the
//! volume's table.
//!
//! D13 (larmorx_core::defects): FS-1249 (a negative offset names entries before the table's
//! first: FreeSurfer writes through a pointer read from before its list), FS-1250 (defect
//! numbers or offsets whose codes pass the `int` range); a point whose voxel coordinate is NaN or
//! beyond the `int` range is outside the volume in both modes (FreeSurfer's conversion gives
//! `INT_MIN`, outside; `nint`'s FS-12 default would give voxel 0).

use larmorx_core::defects::{FS_1249, FS_1250};
use larmorx_freesurfer_core::affine::float_to_int;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::sse::{add64, div64, mul64};
use larmorx_freesurfer_core::utils::nint;
use larmorx_freesurfer_io::colortab::{ctab_alloc, ctab_merge, ctab_unique};
use larmorx_freesurfer_mri::mri::Mri;
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::coords::index_not_in_volume;

use crate::aaa::SurfGlobals;
use crate::coords::Io;
use crate::mp_slow::normal_face;
use crate::mris::Mris;
use crate::sras;

/// What [`mris_defects2seg`] ended with (FreeSurfer's `int`: 0, 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Defects2Seg {
    /// 0: the volume painted, its colour table replaced.
    Done,
    /// 1: the overlay's width is not the surface's vertex count (its message printed).
    Mismatch,
    /// larmorx's own refusals (FS-1249, FS-1250): their messages printed; FreeSurfer goes on
    /// into memory it does not own (`reproduce` gives [`Defects2Seg::Crash`]).
    Refused,
    /// `reproduce` of FS-1249 and FS-1250: FreeSurfer writes through a pointer read from outside
    /// the colour table's list, or allocates a table of a negative size and crashes in
    /// `CTABunique` (exit status 139).
    Crash,
}

/// `nint(x)` of a voxel coordinate, or `None` where the coordinate is NaN or beyond the `int`
/// range (FreeSurfer's `INT_MIN`, which [`index_not_in_volume`] places outside; the same answer in
/// both modes, so no notice).
fn voxel_index(g: &mut Globals, x: f64) -> Option<i32> {
    let v = if x < 0.0 { x - 0.5 } else { x + 0.5 };
    if v.is_nan() || v >= 2147483648.0 || v <= -2147483649.0 {
        return None;
    }
    Some(nint(g, x))
}

/// `MRISdefects2Seg(surf, defects, offset, vol)`: see the module's documentation. `defects` is the
/// overlay of defect numbers (one voxel per vertex, row 0, slice 0, frame 0).
#[allow(clippy::too_many_arguments)]
pub fn mris_defects2seg(
    g: &mut Globals,
    sg: &SurfGlobals,
    io: &mut Io,
    surf: &Mris,
    defects: &Mri,
    offset: i32,
    vol: &mut Mri,
) -> Result<Defects2Seg, Fatal> {
    // delta = vol->xsize / 5.0 (a float's value in double)
    let delta = div64(f64::from(vol.vg.xsize), 5.0);
    let mut map = sras::mris_make_ras2voxel_map(g, io, vol, surf)?;
    if surf.nvertices != defects.vg.width {
        let _ = writeln!(
            io.out,
            "ERROR: MRISdefectNo2Vol(): dimension mismatch surf={}, defects={}",
            surf.nvertices, defects.vg.width
        );
        let _ = io.out.flush();
        return Ok(Defects2Seg::Mismatch);
    }
    let nv = usize::try_from(surf.nvertices).unwrap_or(0);
    let mut numbers = Vec::with_capacity(nv);
    let mut defectnomax = 0i32;
    for n in 0..nv {
        let v = mri_get_vox_val(g, io.err, defects, n as i32, 0, 0, 0, 0)?;
        let defectno = float_to_int(g, f64::from(v), "MRISdefects2Seg: a defect number");
        if defectno > defectnomax {
            defectnomax = defectno;
        }
        numbers.push(defectno);
    }
    // the table's size and the names' places, in a wider type than FreeSurfer's int
    let wide = i64::from(defectnomax) + i64::from(offset) + 2;
    if wide > i64::from(i32::MAX)
        || i64::from(defectnomax) + i64::from(offset) > i64::from(i32::MAX)
    {
        g.notices.add(
            &FS_1250,
            format!(
                "mri_label2vol --defects: defect numbers up to {defectnomax} with offset {offset} pass the int range (FreeSurfer's table size and codes wrap)"
            ),
        );
        if g.notices.reproduce() {
            return Ok(Defects2Seg::Crash);
        }
        let _ = writeln!(
            io.out,
            "ERROR: MRISdefects2Seg(): defect numbers up to {defectnomax} plus the offset {offset} pass 2147483647"
        );
        return Ok(Defects2Seg::Refused);
    }
    if offset < 0 {
        g.notices.add(
            &FS_1249,
            format!(
                "mri_label2vol --defects: offset {offset} names colour-table entries before the first (FreeSurfer writes through a pointer read from before its list)"
            ),
        );
        if g.notices.reproduce() {
            return Ok(Defects2Seg::Crash);
        }
        let _ = writeln!(
            io.out,
            "ERROR: MRISdefects2Seg(): the offset {offset} is negative: the colour table has no entry {offset}"
        );
        return Ok(Defects2Seg::Refused);
    }
    let mut nentries = wide as i32;
    if let Some(ct) = vol.ct.as_ref() {
        nentries = nentries.max(ct.nentries());
    }
    let env = io.env;
    let Some(mut ctab) = ctab_alloc(g, env, io.out, io.err, nentries)? else {
        // CTABalloc refused (its message printed): CTABunique(NULL) crashes in FreeSurfer
        return Ok(Defects2Seg::Crash);
    };
    ctab_unique(g, env, io.out, io.err, &mut ctab, 10)?;
    for n in 0..=defectnomax {
        let k = (n + offset) as usize;
        if let Some(Some(e)) = ctab.entries.get_mut(k) {
            e.name = format!("Defect-{:03}", n + offset).into_bytes();
        }
    }
    if let Some(old) = vol.ct.take() {
        ctab_merge(g, &mut ctab, &old);
    }
    vol.ct = Some(ctab);

    let mut io2 = Io {
        out: &mut *io.out,
        err: &mut *io.err,
        env,
    };
    for (n, &defectno) in numbers.iter().enumerate() {
        if defectno == 0 {
            continue;
        }
        let vt = surf.topology(n);
        let faces = vt.f.as_deref().unwrap_or(&[]);
        for &faceno in faces.iter().take(usize::from(vt.num)) {
            let face = surf.face(faceno as usize);
            let (mut cx, mut cy, mut cz) = (0.0f64, 0.0f64, 0.0f64);
            for k in 0..3 {
                let vf = surf.vertex(face.v[k] as usize);
                cx = add64(cx, f64::from(vf.x));
                cy = add64(cy, f64::from(vf.y));
                cz = add64(cz, f64::from(vf.z));
            }
            cx = div64(cx, 3.0);
            cy = div64(cy, 3.0);
            cz = div64(cz, 3.0);
            let snorm = normal_face(sg, surf, faceno as usize, 0);
            for projsign in [-1.0f64, 1.0] {
                let d = mul64(projsign, delta);
                let x = add64(cx, mul64(d, f64::from(snorm[0])));
                let y = add64(cy, mul64(d, f64::from(snorm[1])));
                let z = add64(cz, mul64(d, f64::from(snorm[2])));
                let [c, r, s] = sras::mris_use_ras2voxel_map(g, &mut io2, &mut map, x, y, z)?;
                let (Some(ic), Some(ir), Some(is)) =
                    (voxel_index(g, c), voxel_index(g, r), voxel_index(g, s))
                else {
                    continue;
                };
                if index_not_in_volume(vol, f64::from(ic), f64::from(ir), f64::from(is)) != 0 {
                    continue;
                }
                let val = defectno.wrapping_add(offset) as f32;
                mri_set_vox_val(g, io2.err, vol, ic, ir, is, 0, val, 0)?;
            }
        }
    }
    sras::mris_free_ras2voxel_map(map);
    Ok(Defects2Seg::Done)
}
