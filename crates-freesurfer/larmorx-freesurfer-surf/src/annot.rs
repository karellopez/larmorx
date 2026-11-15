// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! A surface's annotation files (`fs.surfio`, queue items 3.1-47/48, relay 36 of track D):
//! `utils/mrisurf_io.cpp`'s `MRISreadAnnotation` 1387 (with `__mrisreadannot` 1425 and
//! `__mrisreadseg2annot` 1497) and `MRISwriteAnnotation` 1756 (with `__mriswriteannot` 1805), and
//! the surface pieces they call that wave 3.4 would bring: `MRISclearAnnotations`
//! (`utils/mrisurf_vals.cpp` 386), `MRISseg2annot` (`utils/mrisutils.cpp` 1287) and
//! `MRISannot2seg` (`utils/annotation.cpp` 914).
//!
//! FreeSurfer's annotation table is one per process (`annotation.cpp`'s `atable`): here it is
//! [`SurfGlobals::atable`], which `MRISseg2annot` sets from the colour table and
//! `MRISannot2seg` reads without one. FS-624: the table is set only when empty, so a second
//! segmentation read with another colour table maps its numbers through the first table.

use std::path::Path;

use larmorx_core::defects::{FS_624, FS_625};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADFILE, ERROR_NOFILE, Fatal, NO_ERROR};
use larmorx_freesurfer_io::annotation::{self, AnnotationTables};
use larmorx_freesurfer_io::colortab::ctab_find_annotation;
use larmorx_freesurfer_io::colortab_bin::{ctab_read_from_binary, ctab_write_into_binary};
use larmorx_freesurfer_io::fio::{Slot49, fread_int_in, fread_int_raw, fwrite_int};
use larmorx_freesurfer_io::gifti_common::{NIFTI_INTENT_LABEL, NIFTI_INTENT_RGBA_VECTOR};
use larmorx_freesurfer_io::identify::{GIFTI_FILE, MGH_ANNOT, MRI_MGH_FILE, NII_FILE};
use larmorx_freesurfer_io::mriio;
use larmorx_freesurfer_mri::ctab::{ColorTable, ctab_deep_copy};
use larmorx_freesurfer_mri::mri::{
    MGH_VERSION, MGZ_INTENT_LABEL, MGZ_INTENT_UNKNOWN, MRI_INT, Mri, mri_alloc,
};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

use crate::aaa::SurfGlobals;
use crate::coords::Io;
use crate::gifti::{self, GiftiRead};
use crate::gifti_write;
use crate::mris::Mris;
use crate::names;
use crate::sfile::{c_str, nonneg, open, open_failed};
use crate::surfread::verbose_show;

/// `TAG_OLD_COLORTABLE` (`tags.h`): the tag before an annotation file's colour table.
pub const TAG_OLD_COLORTABLE: i32 = 1;

/// `MRISclearAnnotations(mris)`: every unripped vertex's annotation 0.
pub fn clear_annotations(mris: &mut Mris) -> i32 {
    for vno in 0..nonneg(mris.nvertices) {
        let v = mris.vertex_mut(vno);
        if v.ripflag != 0 {
            continue;
        }
        v.annotation = 0;
    }
    NO_ERROR
}

/// `MRISreadAnnotation(mris, sname, giftiDaNum, ctab)`: the annotation file
/// [`names::read_annotation_getfilename`] chooses for `sname`, the unripped vertices' annotations
/// cleared first ([`clear_annotations`]), by its type: an annotation file ([`read_annot`]), an MGH
/// or NIfTI surface segmentation ([`read_seg2annot`]), a GIFTI file (`mrisReadGIFTIfile` with
/// that data array and colour table: `ERROR_BADFILE` when it gives no surface), else `ERROR:
/// unknown annotation file format <file>` on standard output and `ERROR_BADFILE`. Returns the
/// reader's status.
#[allow(clippy::too_many_arguments)]
pub fn read_annotation(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    sname: &str,
    gifti_da_num: i32,
    ctab: Option<&ColorTable>,
) -> Result<i32, Fatal> {
    let (mritype, annot_to_read) =
        names::read_annotation_getfilename(g, io, dir, Some(mris), sname)?;
    clear_annotations(mris);
    Ok(if mritype == MGH_ANNOT {
        read_annot(g, io, dir, mris, &annot_to_read, ctab)?
    } else if mritype == MRI_MGH_FILE || mritype == NII_FILE {
        read_seg2annot(g, sg, io, dir, mris, &annot_to_read, ctab)?
    } else if mritype == GIFTI_FILE {
        let r = gifti::mris_read_gifti_file(
            g,
            sg,
            io,
            dir,
            &annot_to_read,
            Some(mris),
            None,
            gifti_da_num,
            ctab,
        )?;
        if matches!(r, GiftiRead::Null) {
            ERROR_BADFILE
        } else {
            NO_ERROR
        }
    } else {
        let _ = writeln!(
            io.out,
            "ERROR: unknown annotation file format {annot_to_read}"
        );
        ERROR_BADFILE
    })
}

/// `__mrisreadannot(fannot, mris, ctab)`: an annotation file: the vertex count as a big-endian
/// `int` (`# elements (<n>) in <file> and # vertices (<m>) don't match`, `ERROR_NOFILE`, when it
/// is not the surface's), then per entry a vertex number and its annotation (`vertex out of range:
/// annot file <file> entry <j> (vno=<v> annot=<a>)`, `ERROR_NOFILE`, for a vertex outside the
/// surface; the entries before it kept); a file that cannot be opened: `could not read annot file
/// <file>` (`ERROR_NOFILE`). Then the colour table given replaces the surface's (`NO_ERROR`);
/// else, after the entries, the tag `TAG_OLD_COLORTABLE` and a binary colour table
/// (`CTABreadFromBinary`; with verbose: `reading colortable from annotation file...` and
/// `colortable with <n> entries read (originally <name>)`) become the surface's when it reads; a
/// file that ends before a tag gives `NO_ERROR`. FS-49: the numbers a short file cuts are memory
/// contents in FreeSurfer.
pub fn read_annot(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    fannot: &str,
    ctab: Option<&ColorTable>,
) -> Result<i32, Fatal> {
    let fannot = c_str(fannot);
    let mut fp = match open(dir, fannot, "r") {
        Ok(f) => f,
        Err(e) => {
            let m = format!("could not read annot file {fannot}");
            return Ok(open_failed(g, io, e, ERROR_NOFILE, &m));
        }
    };
    let nvertices = mris.nvertices;
    let mut slot = Slot49::new();
    let nelem = fread_int_in(g, &mut fp, &mut slot);
    if nelem != nvertices {
        let m =
            format!("# elements ({nelem}) in {fannot} and # vertices ({nvertices}) don't match");
        log::error_printf(g, io.err, ERROR_NOFILE, &m);
        return Ok(ERROR_NOFILE);
    }
    for j in 0..nelem.max(0) {
        let vno = fread_int_in(g, &mut fp, &mut slot);
        let annot = fread_int_in(g, &mut fp, &mut slot);
        if vno >= nvertices || vno < 0 {
            let m = format!(
                "vertex out of range: annot file {fannot} entry {j} (vno={vno} annot={annot})\n"
            );
            log::error_printf(g, io.err, ERROR_NOFILE, &m);
            return Ok(ERROR_NOFILE);
        }
        mris.vertex_mut(vno as usize).annotation = annot;
    }
    if let Some(ct) = ctab {
        // FreeSurfer leaves the file open here (nothing else differs)
        mris.ct = Some(ct.clone());
        fp.close();
        return Ok(NO_ERROR);
    }
    let (b, _) = fread_int_raw(&mut fp, &mut Slot49::new());
    if fp.feof() {
        fp.close();
        return Ok(NO_ERROR);
    }
    let tag = i32::from_be_bytes(b);
    if tag == TAG_OLD_COLORTABLE {
        let verbose = larmorx_freesurfer_core::diag::verbose_on(g);
        if verbose {
            let _ = writeln!(io.out, "reading colortable from annotation file...");
        }
        if let Some(ct) = ctab_read_from_binary(g, io.out, io.err, Some(&mut fp)) {
            if verbose {
                let _ = writeln!(
                    io.out,
                    "colortable with {} entries read (originally {})",
                    ct.nentries(),
                    String::from_utf8_lossy(&ct.fname)
                );
            }
            mris.ct = Some(ct);
        }
    }
    fp.close();
    Ok(NO_ERROR)
}

/// `__mrisreadseg2annot(fannot, mris, ctab)`: a surface segmentation (an MGH or NIfTI volume of
/// one voxel per vertex) as the annotation: `__mrisreadseg2annot(): reading <file> as a surface
/// seg` on standard output; the volume read whole (`MRIread`; `ERROR_NOFILE` when it gives none);
/// the colour table given replaces the volume's (`__mrisreadseg2annot: Use Customized
/// Colortable`); without either, `ERROR: __mrisreadseg2annot(): <file> does not have a
/// colortable`; a width other than the vertex count `ERROR: __mrisreadseg2annot(): dimension
/// mismatch <file>=<w>, surf=<n>`; then [`seg2annot`] (`ERROR: __mrisreadseg2annot(): could not
/// convert <file> to an annotation` when it fails), and a copy of the table becomes the surface's.
/// Each failure returns `ERROR_NOFILE`, success `NO_ERROR`. FS-625 (rule A): with a colour table
/// given, FreeSurfer frees that table with the volume on every path after taking it (the caller's
/// table, which a later use then reads freed); larmorx leaves it alone in both modes, noted.
pub fn read_seg2annot(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    fannot: &str,
    ctab: Option<&ColorTable>,
) -> Result<i32, Fatal> {
    let fannot = c_str(fannot);
    let _ = writeln!(
        io.out,
        "__mrisreadseg2annot(): reading {fannot} as a surface seg"
    );
    let Some(mut surfseg) = mriio::mri_read_volume(g, io.out, io.err, io.env, dir, fannot)? else {
        return Ok(ERROR_NOFILE);
    };
    if surfseg.ct.is_none() && ctab.is_none() {
        let _ = writeln!(
            io.out,
            "ERROR: __mrisreadseg2annot(): {fannot} does not have a colortable"
        );
        return Ok(ERROR_NOFILE);
    }
    if let Some(ct) = ctab {
        let _ = writeln!(io.out, "__mrisreadseg2annot: Use Customized Colortable");
        surfseg.ct = Some(ct.clone());
        // every path from here frees the segmentation volume with its table, the caller's
        g.notices.add(
            &FS_625,
            "MRISreadAnnotation: a segmentation read with the caller's colour table leaves that table alone (FreeSurfer frees it: a later use reads freed memory)",
        );
    }
    if surfseg.width != mris.nvertices {
        let _ = writeln!(
            io.out,
            "ERROR: __mrisreadseg2annot(): dimension mismatch {fannot}={}, surf={}",
            surfseg.width, mris.nvertices
        );
        return Ok(ERROR_NOFILE);
    }
    let ct = surfseg.ct.clone();
    if seg2annot(g, sg, io, dir, mris, &surfseg, ct.as_ref())? != 0 {
        let _ = writeln!(
            io.out,
            "ERROR: __mrisreadseg2annot(): could not convert {fannot} to an annotation"
        );
        return Ok(ERROR_NOFILE);
    }
    mris.ct = ctab_deep_copy(g, io.err, surfseg.ct.as_ref());
    Ok(NO_ERROR)
}

/// `MRISseg2annot(mris, surfseg, ctab)`: each vertex's annotation from the segmentation's voxel
/// at its column (`MRIgetVoxVal`, as an `int`) through the annotation table
/// (`index_to_annotation`: 0 for a number it does not hold); the colour table given becomes the
/// surface's (without either: `ERROR: MRISseg2annot: both ctab and mris->ct are NULL` on standard
/// output and 1); the process's table set from the surface's colour table first
/// (`set_atable_from_ctable`: FS-624, rule A: only when the process has none yet, so a later
/// segmentation with another colour table is mapped through the first one; larmorx maps it
/// through its own, the process's table kept as FreeSurfer keeps it; `reproduce` FreeSurfer's).
/// `WARNING: MRISseg2annot: no voxels matching ctab` when no voxel that is not 0 had an entry,
/// `WARNING: MRISseg2annot: found <n> voxels with no entry in ctab` when some had none. With
/// `Gdiag_no` a vertex: `<vno> <segid> <annot> <name>` (`%5d %2d %2d %s`). Returns 0.
pub fn seg2annot(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    surfseg: &Mri,
    ctab: Option<&ColorTable>,
) -> Result<i32, Fatal> {
    match ctab {
        None => {
            if mris.ct.is_none() {
                let _ = writeln!(
                    io.out,
                    "ERROR: MRISseg2annot: both ctab and mris->ct are NULL"
                );
                return Ok(1);
            }
        }
        Some(ct) => mris.ct = Some(ct.clone()),
    }
    let had = sg.atable.num_entries > 0;
    annotation::set_atable_from_ctable(g, io.out, io.err, &mut sg.atable, mris.ct.as_ref());
    // FS-624: an earlier table kept; larmorx maps through the surface's own colour table
    let mut own = AnnotationTables::default();
    let fresh = if had {
        annotation::set_atable_from_ctable(
            g,
            &mut Vec::new(),
            &mut Vec::new(),
            &mut own,
            mris.ct.as_ref(),
        );
        if own.atable != sg.atable.atable || own.num_entries != sg.atable.num_entries {
            g.notices.add(
                &FS_624,
                "MRISseg2annot: a segmentation mapped through its own colour table (FreeSurfer uses the first table the program set)",
            );
            !g.notices.reproduce()
        } else {
            false
        }
    } else {
        false
    };
    let (mut nhits, mut nmisses) = (0i32, 0i32);
    let nv = mris.nvertices;
    for vtxno in 0..nonneg(nv) {
        let segid = mri_get_vox_val(g, io.err, surfseg, vtxno as i32, 0, 0, 0, 0)?;
        let segid = larmorx_freesurfer_core::cstd::double_to_int_x86(f64::from(segid));
        let t: &mut AnnotationTables = if fresh { &mut own } else { &mut sg.atable };
        let mut ano = annotation::index_to_annotation(g, io.env, io.out, io.err, dir, t, segid);
        if ano != -1 && segid != 0 {
            nhits += 1;
        }
        if ano == -1 && segid != 0 {
            nmisses += 1;
        }
        if ano == -1 {
            ano = 0;
        }
        mris.vertex_mut(vtxno).annotation = ano;
        if vtxno as i32 == g.gdiag_no {
            let t: &mut AnnotationTables = if fresh { &mut own } else { &mut sg.atable };
            let name = annotation::index_to_name(g, io.env, io.out, io.err, dir, t, segid);
            let _ = writeln!(
                io.out,
                "{vtxno:5} {segid:2} {ano:2} {}",
                String::from_utf8_lossy(&name)
            );
        }
    }
    if nhits == 0 {
        let _ = writeln!(io.out, "WARNING: MRISseg2annot: no voxels matching ctab");
    }
    if nmisses > 0 {
        let _ = writeln!(
            io.out,
            "WARNING: MRISseg2annot: found {nmisses} voxels with no entry in ctab"
        );
    }
    Ok(0)
}

/// `MRISannot2seg(surf, base)`: an `int` volume of `<vertices>` x 1 x 1 (a copy of the surface's
/// colour table, its voxel sizes, directions and centre from the surface's volume geometry), each
/// vertex's voxel the entry of its annotation's colour in the surface's colour table (without one,
/// in the process's annotation table, `annotation_to_index`), 0 when there is none, plus `base`;
/// `MRISannot2seg(): nhits = <n>` (the vertices whose annotation is not 0) on standard output.
pub fn annot2seg(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    surf: &Mris,
    base: i32,
) -> Result<Mri, Fatal> {
    let mut seg = mri_alloc(g, io.out, io.err, surf.nvertices, 1, 1, MRI_INT)?;
    if surf.ct.is_some() {
        seg.ct = ctab_deep_copy(g, io.err, surf.ct.as_ref());
    }
    let (vg, sv) = (&surf.vg, &mut seg.vg);
    (sv.xsize, sv.ysize, sv.zsize) = (vg.xsize, vg.ysize, vg.zsize);
    (sv.x_r, sv.x_a, sv.x_s) = (vg.x_r, vg.x_a, vg.x_s);
    (sv.y_r, sv.y_a, sv.y_s) = (vg.y_r, vg.y_a, vg.y_s);
    (sv.z_r, sv.z_a, sv.z_s) = (vg.z_r, vg.z_a, vg.z_s);
    (sv.c_r, sv.c_a, sv.c_s) = (vg.c_r, vg.c_a, vg.c_s);
    let mut nhits = 0i32;
    for k in 0..nonneg(surf.nvertices) {
        let annot = surf.vertex(k).annotation;
        if annot != 0 {
            nhits += 1;
        }
        let mut segid = match &surf.ct {
            Some(ct) => ctab_find_annotation(ct, annot),
            None => annotation::annotation_to_index(
                g,
                io.env,
                io.out,
                io.err,
                dir,
                &mut sg.atable,
                annot,
            ),
        };
        if segid == -1 {
            segid = 0;
        }
        mri_set_vox_val(
            g,
            io.err,
            &mut seg,
            k as i32,
            0,
            0,
            0,
            (segid + base) as f32,
            0,
        )?;
    }
    let _ = writeln!(io.out, "MRISannot2seg(): nhits = {nhits}");
    Ok(seg)
}

/// `MRISwriteAnnotation(mris, sname, writect)`: the vertices' annotations into the file
/// [`names::write_annotation_getfilename`] chooses for `sname`, by its type: an annotation file
/// ([`write_annot`]); GIFTI (`MRISwriteGIFTI` with the label intent when `writect`, else the
/// RGBA vector intent); MGH or NIfTI: `MRISurfAnnotation::writeAnnotation((): writing <file> as a
/// surface seg` on standard output, [`annot2seg`] with base 0, its version the label intent's
/// (`(1 << 8) | 1`), written by `MRIwrite`; else `ERROR: unknown output annotation file format
/// <file>` on standard output and `ERROR_BADFILE`. Returns the writer's status.
pub fn write_annotation(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    sname: &str,
    writect: bool,
) -> Result<i32, Fatal> {
    let (mritype, annot_to_write) = names::write_annotation_getfilename(g, io, dir, mris, sname)?;
    Ok(if mritype == MGH_ANNOT {
        write_annot(g, io, dir, mris, &annot_to_write)
    } else if mritype == GIFTI_FILE {
        let intent = if writect {
            NIFTI_INTENT_LABEL
        } else {
            NIFTI_INTENT_RGBA_VECTOR
        };
        gifti_write::mris_write_gifti(g, io, dir, mris, intent, &annot_to_write, None)?
    } else if mritype == MRI_MGH_FILE || mritype == NII_FILE {
        let _ = writeln!(
            io.out,
            "MRISurfAnnotation::writeAnnotation((): writing {annot_to_write} as a surface seg"
        );
        let mut surfseg = annot2seg(g, sg, io, dir, mris, 0)?;
        surfseg.version = ((MGZ_INTENT_LABEL & 0xffff) << 8) | MGH_VERSION;
        mriio::mri_write(
            g,
            io.out,
            io.err,
            io.env,
            dir,
            &mut surfseg,
            &annot_to_write,
            None,
            MGZ_INTENT_UNKNOWN,
        )?
    } else {
        let _ = writeln!(
            io.out,
            "ERROR: unknown output annotation file format {annot_to_write}"
        );
        ERROR_BADFILE
    })
}

/// `__mriswriteannot(mris, outfannot)`: the vertex count, then each vertex's number and
/// annotation as big-endian `int`s; with a colour table, the tag `TAG_OLD_COLORTABLE` and the
/// table (`CTABwriteIntoBinary`; with `DIAG_SHOW` and verbose: `writing colortable into
/// annotation file...` first). Returns `NO_ERROR`, or `ERROR_NOFILE` after `could not write annot
/// file <file>` when the file cannot be created.
pub fn write_annot(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &Mris,
    outfannot: &str,
) -> i32 {
    let outfannot = c_str(outfannot);
    let mut fp = match open(dir, outfannot, "wb") {
        Ok(f) => f,
        Err(e) => {
            let m = format!("could not write annot file {outfannot}");
            return open_failed(g, io, e, ERROR_NOFILE, &m);
        }
    };
    fwrite_int(&mut fp, mris.nvertices);
    for vno in 0..nonneg(mris.nvertices) {
        fwrite_int(&mut fp, vno as i32);
        fwrite_int(&mut fp, mris.vertex(vno).annotation);
    }
    if let Some(ct) = &mris.ct {
        if verbose_show(g) {
            let _ = writeln!(io.out, "writing colortable into annotation file...");
        }
        fwrite_int(&mut fp, TAG_OLD_COLORTABLE);
        ctab_write_into_binary(g, io.out, io.err, Some(ct), Some(&mut fp));
    }
    fp.close();
    NO_ERROR
}
