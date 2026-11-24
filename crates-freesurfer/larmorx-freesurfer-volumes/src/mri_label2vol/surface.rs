// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The surface library's functions `mri_label2vol` reaches that no other ported program does:
//! FreeSurfer 8.2.0's `MRISloadSurfSubject` (`utils/mrisutils.cpp` 957-981), `ProjNormFracThick`
//! (`utils/resample.cpp` 626-634), `ProjNormDist` (659-665) and `annotation2label`
//! (`utils/annotation.cpp` 318-367). They move to fs.surf when a later program needs them
//! (`mri_vol2surf`, `mri_surf2vol`, `mri_annotation2label`).

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::sse::{add, mul};
use larmorx_freesurfer_io::annotation;
use larmorx_freesurfer_io::colortab::ctab_find_annotation;
use larmorx_freesurfer_io::label::{Label, label_alloc};
use larmorx_freesurfer_io::streams::Io;
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use larmorx_freesurfer_surf::mris::Mris;
use larmorx_freesurfer_surf::readwrite;

use super::main::Run;

/// `MRISloadSurfSubject(subj, hemi, surfid, SUBJECTS_DIR)`: `<SUBJECTS_DIR>/<subj>/surf/<hemi>.
/// <surfid>` (`  INFO: loading surface  <name>` on standard output), read with `MRISread`; a
/// surface that does not read ends the program (`ERROR: could not load registration surface`,
/// status 1); then `nvertices = <n>`. `mri_label2vol` always passes a `SUBJECTS_DIR`.
pub fn mris_load_surf_subject(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    r: &mut Run,
    subj: &[u8],
    hemi: &str,
    surfid: &str,
    subjects_dir: &str,
) -> Result<Mris, Fatal> {
    let fname = format!(
        "{subjects_dir}/{}/surf/{hemi}.{surfid}",
        String::from_utf8_lossy(subj)
    );
    let _ = writeln!(r.out, "  INFO: loading surface  {fname}");
    let _ = r.out.flush();
    let surf = {
        let mut io = Io {
            out: &mut *r.out,
            err: &mut *r.err,
            env: &r.env,
        };
        readwrite::read(g, sg, &mut io, r.dir, &fname, true)?
    };
    let Some(surf) = surf else {
        let _ = writeln!(r.out, "ERROR: could not load registration surface");
        return Err(Fatal::Exit(1));
    };
    let _ = writeln!(r.out, "nvertices = {}", surf.nvertices);
    let _ = r.out.flush();
    Ok(surf)
}

/// `ProjNormFracThick(&x, &y, &z, surf, vtx, frac)`: the vertex moved along its normal by `frac`
/// times its `curv` (the thickness), in `float`: `r = frac curv`, `x + r nx`, ...
pub fn proj_norm_frac_thick(surf: &Mris, vtx: usize, frac: f32) -> [f32; 3] {
    let v = surf.vertex(vtx);
    let r = mul(frac, v.curv);
    [
        add(v.x, mul(r, v.nx)),
        add(v.y, mul(r, v.ny)),
        add(v.z, mul(r, v.nz)),
    ]
}

/// `ProjNormDist(&x, &y, &z, surf, vtx, dist)`: the vertex moved along its normal by `dist`, in
/// `float`: `x + dist nx`, ...
pub fn proj_norm_dist(surf: &Mris, vtx: usize, dist: f32) -> [f32; 3] {
    let v = surf.vertex(vtx);
    [
        add(v.x, mul(dist, v.nx)),
        add(v.y, mul(dist, v.ny)),
        add(v.z, mul(dist, v.nz)),
    ]
}

/// The annotation index of a vertex's annotation: the surface's colour table's entry
/// (`CTABfindAnnotation`, -1 when none matches), else FreeSurfer's default table
/// (`annotation_to_index`).
pub fn annotation_index(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    r: &mut Run,
    surf: &Mris,
    annot: i32,
) -> i32 {
    match surf.ct.as_ref() {
        Some(ct) => ctab_find_annotation(ct, annot),
        None => {
            let env = r.env.clone();
            annotation::annotation_to_index(g, &env, r.out, r.err, r.dir, &mut sg.atable, annot)
        }
    }
}

/// `annotation2label(annotid, Surf)`: the label of the vertices whose annotation index is
/// `annotid` ([`annotation_index`]), each with its vertex number and current position, `None`
/// when there is none. The label is `LabelAlloc(n, "", "")`'s: a subject name (empty), so
/// `SUBJECTS_DIR` must be set (its message and status otherwise).
pub fn annotation2label(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    r: &mut Run,
    annotid: i32,
    surf: &Mris,
) -> Result<Option<Label>, Fatal> {
    let nv = usize::try_from(surf.nvertices).unwrap_or(0);
    let mut hits = Vec::new();
    for vtxno in 0..nv {
        let annot = surf.vertex(vtxno).annotation;
        if annotation_index(g, sg, r, surf, annot) == annotid {
            hits.push(vtxno);
        }
    }
    if hits.is_empty() {
        return Ok(None);
    }
    // the second pass of FreeSurfer's: the same indices again (a table read each time)
    let env = r.env.clone();
    let mut label = label_alloc(g, &env, r.err, hits.len() as i32, Some(b""), Some(b""))?;
    label.n_points = hits.len() as i32;
    for (k, &vtxno) in hits.iter().enumerate() {
        let v = surf.vertex(vtxno);
        label.lv[k].vno = vtxno as i32;
        label.lv[k].x = v.x;
        label.lv[k].y = v.y;
        label.lv[k].z = v.z;
    }
    Ok(Some(label))
}
