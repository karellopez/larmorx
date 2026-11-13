// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `utils/gifti.cpp`'s GIFTI files as volumes (queue item 3.1-26; relay 22 of track D):
//! `MRISreadGiftiAsMRI` (1231-1411), which fs.io's `MRIread` calls for a `.gii` name, and
//! `mriWriteGifti` (1506-1609), which `MRIwrite` calls. A surface's per-vertex values are a volume
//! of one column per vertex, one row, one slice, one frame per data array.
//!
//! The defects (`larmorx_core::defects`; mechanisms in `docs/findings/freesurfer-surfio.md` §24):
//! FS-521 (a file mixing intents), FS-522 (a volume of more than one row or slice written), FS-395
//! (a file without a Version).

use std::path::Path;

use larmorx_core::cfmt;
use larmorx_core::defects::{FS_521, FS_522};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{ERROR_NOMEMORY, Fatal};
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_INT, Mri, mri_alloc_header, mri_alloc_sequence};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_io::gifti::{
    GiftiImage, NIFTI_INTENT_NONE, NIFTI_INTENT_TIME_SERIES, NIFTI_TYPE_FLOAT32, intent_to_string,
};

use crate::aaa::SurfGlobals;
use crate::coords::Io;
use crate::gifti::{
    NIFTI_INTENT_LABEL, NIFTI_INTENT_SHAPE, meta, read_label_table, scan_f, value, vertices_cols,
};
use crate::gifti_write::{
    add_array, alloc_data, free, gifti_set_da_value_2d, new_image, read_valid, set_attributes,
    set_sizes, validate_and_write,
};

/// `NIFTI_INTENT_NORMAL` (`nifti1.h`).
const NIFTI_INTENT_NORMAL: i32 = 11;
/// `MGZ_INTENT_*` and `MGH_VERSION` (`mri.h`, `mghendian.h`): the volume's intent and version.
pub const MGZ_INTENT_MRI: i32 = 0;
pub const MGZ_INTENT_LABEL: i32 = 1;
pub const MGZ_INTENT_SHAPE: i32 = 2;
pub const MGZ_INTENT_TIME_SERIES: i32 = 5;
pub const MGH_VERSION: i32 = 1;
/// The intents `MRISreadGiftiAsMRI` looks for, in its order.
const INTENTS: [i32; 5] = [
    NIFTI_INTENT_TIME_SERIES,
    NIFTI_INTENT_SHAPE,
    NIFTI_INTENT_NONE,
    NIFTI_INTENT_NORMAL,
    NIFTI_INTENT_LABEL,
];
/// FS-521 with `reproduce`: the intent FreeSurfer reads from memory past its list, which nothing
/// wrote; larmorx fills it with a value no array has (`gifti_intent_to_string` gives `Unknown`),
/// as the official build reads it in the probe.
pub const FS521_UNWRITTEN_INTENT: i32 = -1;

/// What `MRISreadGiftiAsMRI`'s search found: the chosen intent, the arrays counted, the vertices.
struct Found {
    intent: i32,
    frames: i32,
    num_vertices: i64,
}

/// The search of `MRISreadGiftiAsMRI`: for each intent of [`INTENTS`] in order, its arrays by
/// occurrence (`gifti_find_DA`); every array after the first must have the first's rows and one
/// column, else `MRISreadGiftiAsMRI: malformed time-series data array in file <f>: nvertices=<n>
/// ncols=<c> expected num_vertices=<m>` and `None` (`ncols` prints the first array's columns, as
/// FreeSurfer's message does). FS-521: FreeSurfer stops at an intent only when every array of the
/// file has it; otherwise it counts the arrays of every intent and then reads an intent past its
/// list (memory never written). By default the first intent found is chosen and only its arrays
/// counted; with `reproduce` the counts go on and the intent is [`FS521_UNWRITTEN_INTENT`].
fn search(
    g: &mut Globals,
    sg: &SurfGlobals,
    io: &mut Io,
    image: &GiftiImage,
    fname: &str,
) -> Result<Option<Found>, Fatal> {
    let gx = &sg.gifti;
    let num_da = image.num_da;
    let mut frames = 0;
    let mut num_vertices: i64 = -1;
    let mut num_cols: i64 = 0;
    for &intent in &INTENTS {
        let mut da_num = 0;
        let mut found_here = 0;
        loop {
            let da = {
                let mut w = larmorx_core::cfmt::Writers {
                    out: &mut *io.out,
                    err: &mut *io.err,
                };
                gx.find_da(image, intent, da_num, &mut w)
            };
            let Some(da) = da else { break };
            frames += 1;
            found_here += 1;
            let (nv, nc) = vertices_cols(gx, io, da);
            if num_vertices == -1 {
                num_vertices = nv;
                num_cols = nc;
            } else if num_vertices <= 0 || num_vertices != nv || nc != 1 {
                let _ = writeln!(
                    io.err,
                    "MRISreadGiftiAsMRI: malformed time-series data array in file {fname}: nvertices={} ncols={} expected num_vertices={}",
                    nv as i32, num_cols as i32, num_vertices as i32
                );
                return Ok(None);
            }
            da_num += 1;
            if da_num >= num_da {
                // every array has this intent
                return Ok(Some(Found {
                    intent,
                    frames,
                    num_vertices,
                }));
            }
        }
        if found_here > 0 {
            g.notices.add(
                &FS_521,
                format!(
                    "MRISreadGiftiAsMRI: {fname} mixes intents: its {found_here} arrays of {} read (FreeSurfer reads an intent from memory it never wrote)",
                    intent_to_string(intent)
                ),
            );
            if !g.notices.reproduce() {
                return Ok(Some(Found {
                    intent,
                    frames,
                    num_vertices,
                }));
            }
        }
    }
    if frames == 0 {
        return Ok(Some(Found {
            intent: 0,
            frames,
            num_vertices,
        }));
    }
    Ok(Some(Found {
        intent: FS521_UNWRITTEN_INTENT,
        frames,
        num_vertices,
    }))
}

/// `MRISreadGiftiAsMRI(fname, read_volume)`: the data arrays of a GIFTI file as a volume of one
/// column per vertex, one frame per array, of the first intent the file has among
/// `NIFTI_INTENT_TIME_SERIES`, `_SHAPE`, `_NONE`, `_NORMAL` and `_LABEL` ([`search`]); `None` after
/// `MRISreadGiftiAsMRI: gifti_read_image() returned NULL`, gifticlib's check (`... GIFTI file <f>
/// is invalid!`), a malformed array, or `MRISreadGiftiAsMRI: no overlay data found in file <f>`.
/// Prints `INFO: Found <n> <intent> DataArray` on standard output. Labels: `MRI_INT` voxels, the
/// volume's intent `MGZ_INTENT_LABEL`, its colour table from the file's label table
/// (`readLabelTable`), a value of -1 read as 0; shapes `MGZ_INTENT_SHAPE`; time series
/// `MGZ_INTENT_TIME_SERIES`; the others `MGZ_INTENT_MRI`; the version `(intent << 8) | 1`. Without
/// `read_volume` the header only (`MRI_FLOAT` whatever the intent). The repetition time from the
/// first array's `TimeStep` (`%f`). FS-521, FS-395.
pub fn mris_read_gifti_as_mri(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    fname: &str,
    read_volume: bool,
) -> Result<Option<Mri>, Fatal> {
    let Some(mut image) = read_valid(g, sg, io, dir, fname, "MRISreadGiftiAsMRI")? else {
        return Ok(None);
    };
    let Some(found) = search(g, sg, io, &image, fname)? else {
        free(&sg.gifti, io, image);
        return Ok(None);
    };
    if found.frames == 0 {
        let _ = writeln!(
            io.err,
            "MRISreadGiftiAsMRI: no overlay data found in file {fname}"
        );
        free(&sg.gifti, io, image);
        return Ok(None);
    }
    let chosen = found.intent;
    let mut ct = None;
    let mut intent = MGZ_INTENT_MRI;
    let mut version = MGH_VERSION;
    let mut dtype = MRI_FLOAT;
    if chosen == NIFTI_INTENT_LABEL {
        intent = MGZ_INTENT_LABEL;
        version = ((intent & 0xffff) << 8) | MGH_VERSION;
        dtype = MRI_INT;
        ct = read_label_table(g, sg, io, &mut image, fname)?;
    } else if chosen == NIFTI_INTENT_SHAPE {
        intent = MGZ_INTENT_SHAPE;
        version = ((intent & 0xffff) << 8) | MGH_VERSION;
    } else if chosen == NIFTI_INTENT_TIME_SERIES {
        intent = MGZ_INTENT_TIME_SERIES;
        version = ((intent & 0xffff) << 8) | MGH_VERSION;
    }
    let _ = writeln!(
        io.out,
        "INFO: Found {} {} DataArray",
        found.frames,
        intent_to_string(chosen)
    );
    let width = found.num_vertices as i32;
    let time_step = |g: &mut Globals, io: &mut Io, image: &GiftiImage| -> Option<Vec<u8>> {
        let da = image.darray.first().and_then(|d| d.as_ref())?;
        meta(&sg.gifti, io, &mut g.notices, &da.meta, "TimeStep")
    };
    if !read_volume {
        let mut mri = mri_alloc_header(g, io.out, io.err, width, 1, 1, MRI_FLOAT, found.frames)?;
        mri.nframes = found.frames;
        mri.intent = intent;
        mri.version = version;
        mri.ct = ct;
        if image.num_da > 0
            && let Some(s) = time_step(g, io, &image)
            && let Some(tr) = scan_f(g, &s)
        {
            mri.tr = tr;
        }
        // FreeSurfer returns here without freeing the image (nothing printed)
        return Ok(Some(mri));
    }
    let mut mri = mri_alloc_sequence(g, io.out, io.err, width, 1, 1, dtype, found.frames)?;
    mri.intent = intent;
    mri.version = version;
    mri.ct = ct;
    let mut frame = 0;
    for da_num in 0..image.num_da.max(0) {
        let da = {
            let mut w = larmorx_core::cfmt::Writers {
                out: &mut *io.out,
                err: &mut *io.err,
            };
            sg.gifti.find_da(&image, chosen, da_num, &mut w)
        };
        let Some(da) = da else { continue };
        for vno in 0..found.num_vertices.max(0) {
            let mut val = value(g, io, da, vno as i32, 0)? as f32;
            if chosen == NIFTI_INTENT_LABEL && val == -1.0 {
                val = 0.0;
            }
            mri_set_vox_val(g, io.err, &mut mri, vno as i32, 0, 0, frame, val, 0)?;
        }
        frame += 1;
    }
    if image.num_da > 0
        && let Some(s) = time_step(g, io, &image)
        && let Some(tr) = scan_f(g, &s)
    {
        mri.tr = tr;
    }
    free(&sg.gifti, io, image);
    Ok(Some(mri))
}

/// `mriWriteGifti(mri, out_fname)`: a volume's frames as a GIFTI file, one FLOAT32 data array per
/// frame of one value per voxel (`NIFTI_INTENT_NONE` for one frame, `NIFTI_INTENT_TIME_SERIES`
/// with `TimeStep` (`%f` of the repetition time) for several), with the image's `UserName` and
/// `Date`; checked and written as `MRISwriteGIFTI` does (`mriWriteGifti: GIFTI file <f> is
/// invalid!`, `mriWriteGifti: couldn't write image`). FS-522: FreeSurfer writes the first row of
/// the first slice only (the arrays' length the volume's width); by default every voxel, column by
/// column, row by row, slice by slice (`reproduce`: the first row).
pub fn mri_write_gifti(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    mri: &Mri,
    out_fname: &str,
) -> Result<i32, Fatal> {
    let mut image = new_image(&sg.gifti, io, &mut g.notices);
    let rows = mri.height > 1 || mri.depth > 1;
    if rows {
        g.notices.add(
            &FS_522,
            format!(
                "mriWriteGifti: a volume of {} x {} x {} voxels written whole (FreeSurfer writes its first row only)",
                mri.width, mri.height, mri.depth
            ),
        );
    }
    let whole = rows && !g.notices.reproduce();
    let (height, depth) = if whole {
        (mri.height, mri.depth)
    } else {
        (1, 1)
    };
    let n = i64::from(mri.width) * i64::from(height) * i64::from(depth);
    let n = i32::try_from(n).unwrap_or(i32::MAX);
    let fix = g.notices.fix();
    for frame in 0..mri.nframes.max(0) {
        let Some(k) = add_array(&sg.gifti, io, &mut image) else {
            let _ = io
                .err
                .write_all(b"mriWriteGifti: couldn't allocate giiDataArray\n");
            free(&sg.gifti, io, image);
            return Ok(ERROR_NOMEMORY);
        };
        let da = image.darray[k].as_mut().expect("an added data array");
        let intent = if mri.nframes > 1 {
            NIFTI_INTENT_TIME_SERIES
        } else {
            NIFTI_INTENT_NONE
        };
        da.intent = intent;
        if intent == NIFTI_INTENT_TIME_SERIES {
            let tr = cfmt::f(f64::from(mri.tr), 6);
            let mut w = larmorx_core::cfmt::Writers {
                out: &mut *io.out,
                err: &mut *io.err,
            };
            sg.gifti.add_to_meta(
                &mut da.meta,
                b"TimeStep",
                tr.as_bytes(),
                true,
                &mut w,
                &mut g.notices,
            );
        }
        set_attributes(da, intent, NIFTI_TYPE_FLOAT32, 1, n, 0);
        set_sizes(&sg.gifti, io, fix, da);
        alloc_data(da);
        let mut i = 0;
        for s in 0..depth {
            for r in 0..height {
                for c in 0..mri.width {
                    let v = mri_get_vox_val(g, io.err, mri, c, r, s, frame, 0)?;
                    gifti_set_da_value_2d(g, io.err, da, i, 0, f64::from(v))?;
                    i += 1;
                }
            }
        }
    }
    Ok(validate_and_write(
        g,
        &mut sg.gifti,
        io,
        dir,
        image,
        out_fname,
        "mriWriteGifti",
    ))
}
