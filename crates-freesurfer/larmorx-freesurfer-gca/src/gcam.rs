// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! GCA morphs (`GCA_MORPH`, `include/gcamorph.h`; `utils/gcamorph.cpp`, `utils/warpfield.cpp`)
//! of FreeSurfer 8.2.0, as far as `mri_read` reads one as a volume (track A's relay 38 of
//! FreeSurfer's foundations): `GCAMalloc` (1319), `GCAMread` (1110) with `__m3zRead` (1158) and
//! `Warpfield::read` (warpfield.cpp 412), `GCAMcomputeOriginalProperties` (9354),
//! `GCAMstoreMetricProperties` (9330), `gcamComputeMetricProperties` (3460),
//! `GCAMcopyNodePositions` (9365), `GCAMrasToVox` (11184), `GCAMwriteWarpToMRI` (15494, the new
//! volume), `GCAMwriteMRI` (7797, the new volume), `GCAMinvert` (7501), `GCAMmorphFieldFromAtlas`
//! (10524) and `mri_read`'s branch of type `MGH_MORPH` (mriio.cpp 656-692).
//!
//! A morph is a grid of nodes (`x` slowest, then `y`, then `z`), each with its original position
//! (`origx`, ...: the atlas voxel it was made at), its current position (`x`, ...: where it maps to
//! in the image, in voxels), its node coordinates, label, validity and the volumes of the two
//! parallelepipeds its neighbours span (`area1` towards the next nodes, `area2` towards the
//! previous ones, `area` their mean).
//!
//! Defects (`larmorx_core::defects`; `docs/findings/freesurfer-io.md` Part 33): FS-851 (a header
//! that asks for more nodes than the machine's memory), FS-852 (`SHOW_NEG`'s lines under several
//! threads), FS-853 (the spacing's cube past 32 bits), FS-854 (a compressed morph damaged after
//! its nodes), FS-167 (numbers the file cuts short).

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::{FS_851, FS_852, FS_853, FS_854, FS_855};
use larmorx_freesurfer_core::alloc::MACHINE_BYTES;
use larmorx_core::math::Libm;
use larmorx_freesurfer_core::cstd::{fmt_f, note_glibc07};
use larmorx_freesurfer_core::diag::{BUILD_ROOT, DIAG_SHOW, DIAG_WRITE, verbose_on};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{
    self, ERROR_BADFILE, ERROR_BADPARM, ERROR_NOMEMORY, ERROR_UNSUPPORTED, Fatal, NO_ERROR,
};
use larmorx_freesurfer_core::matrix::{Matrix, matrix_copy, matrix_multiply, matrix_multiply_d};
use larmorx_freesurfer_core::matrix_ops::{matrix_determinant, matrix_inverse, matrix_print};
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_core::utils::{self, nint};
use larmorx_freesurfer_filter::norm::{CONTROL_MARKED, mri_build_voronoi_diagram, mri_soap_bubble};
use larmorx_freesurfer_io::{fio, identify, mriio, tags};
use larmorx_freesurfer_mri::mri::{
    MGH_VERSION, MGZ_INTENT_WARPMAP, MRI_FLOAT, MRI_UCHAR, Mri, mri_alloc, mri_alloc_header,
    mri_alloc_sequence, mri_copy_header, mri_copy_vol_geom_to_mri,
};
use larmorx_freesurfer_mri::vol_geom::{self, VolGeom, init_vol_geom, xfm_crs2xyz};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::arith::Pair;
use larmorx_freesurfer_volume::frames::mri_clone_different_type;
use larmorx_freesurfer_volume::interpolate_into::mri_interpolate_into_volume;
use larmorx_freesurfer_volume::resize::mri_set_resolution;
use larmorx_freesurfer_volume::vgeom::{vg_get_voxel_to_ras_xform, vol_geom_text};
use larmorx_io::znz::ZnzFile;

use crate::cma_names::NAMES;

/// `GCAM_VERSION`: the only version `__m3zRead` takes.
pub const GCAM_VERSION: f64 = 1.0;
/// `gcam->type`: positions in RAS or in voxels.
pub const GCAM_RAS: i32 = 1;
pub const GCAM_VOX: i32 = 2;
/// `gcam->status`.
pub const GCAM_UNLABELED: i32 = 0;
pub const GCAM_LABELED: i32 = 1;
/// A node's `invalid`.
pub const GCAM_VALID: i8 = 0;
pub const GCAM_AREA_INVALID: i8 = 1;
pub const GCAM_POSITION_INVALID: i8 = 2;
/// `GCAMcopyNodePositions`' positions.
pub const ORIGINAL_POSITIONS: i32 = 0;
pub const SAVED_POSITIONS: i32 = 1;
pub const CURRENT_POSITIONS: i32 = 2;
pub const SAVED_ORIGINAL_POSITIONS: i32 = 3;
pub const SAVED2_POSITIONS: i32 = 4;
/// `GCAMwriteMRI`'s and `GCAMmorphFieldFromAtlas`' fields (`which`), and `mri_read`'s frame
/// numbers of a morph.
pub const GCAM_X_GRAD: i32 = 0;
pub const GCAM_Y_GRAD: i32 = 1;
pub const GCAM_Z_GRAD: i32 = 2;
pub const GCAM_LABEL: i32 = 3;
pub const GCAM_ORIGX: i32 = 4;
pub const GCAM_ORIGY: i32 = 5;
pub const GCAM_ORIGZ: i32 = 6;
pub const GCAM_NODEX: i32 = 7;
pub const GCAM_NODEY: i32 = 8;
pub const GCAM_NODEZ: i32 = 9;
pub const GCAM_MEANS: i32 = 10;
pub const GCAM_COVARS: i32 = 11;
pub const GCAM_DIAG_VOL: i32 = 12;
pub const GCAM_X: i32 = 13;
pub const GCAM_Y: i32 = 14;
pub const GCAM_Z: i32 = 15;
pub const GCAM_JACOBIAN: i32 = 16;
pub const GCAM_AREA: i32 = 17;
pub const GCAM_ORIG_AREA: i32 = 18;
pub const GCAM_NEG: i32 = 19;
pub const GCAM_MIN_AREA: i32 = 20;
pub const GCAM_LOG_MIN_AREA: i32 = 21;
pub const GCAM_LOG_JACOBIAN: i32 = 22;
pub const GCAM_STATUS: i32 = 23;
pub const GCAM_INVALID: i32 = 24;

/// `WarpfieldDTFMT`'s formats of a warp volume's three frames.
const WARPFIELD_DTFMT_ABS_CRS: i32 = 0;
const WARPFIELD_DTFMT_DISP_CRS: i32 = 1;
const WARPFIELD_DTFMT_ABS_RAS: i32 = 2;
const WARPFIELD_DTFMT_DISP_RAS: i32 = 3;

/// `sizeof(GCA_MORPH_NODE)` in FreeSurfer's build (x86-64), `sizeof(GCA_MORPH_NODE *)`.
const SIZEOF_NODE: u64 = 264;
const SIZEOF_PTR: u64 = 8;

/// `FLT_EPSILON` as `FZERO` compares with it.
const FLT_EPSILON: f64 = f32::EPSILON as f64;

fn fzero(x: f64) -> bool {
    x.abs() < FLT_EPSILON
}

/// `GCA_MORPH_NODE`: one node of the morph (FreeSurfer's fields; `gc`, the node's class in an
/// atlas, is absent: no morph read from a file has one).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GcamNode {
    pub invalid: i8,
    pub label: i32,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub origx: f64,
    pub origy: f64,
    pub origz: f64,
    pub saved_origx: f64,
    pub saved_origy: f64,
    pub saved_origz: f64,
    pub xs: f64,
    pub ys: f64,
    pub zs: f64,
    pub xs2: f64,
    pub ys2: f64,
    pub zs2: f64,
    pub xn: i32,
    pub yn: i32,
    pub zn: i32,
    pub n: i32,
    pub prior: f32,
    pub log_p: f32,
    pub dx: f32,
    pub dy: f32,
    pub dz: f32,
    pub odx: f32,
    pub ody: f32,
    pub odz: f32,
    pub jx: f32,
    pub jy: f32,
    pub jz: f32,
    pub area: f32,
    pub area1: f32,
    pub area2: f32,
    pub orig_area: f32,
    pub orig_area1: f32,
    pub orig_area2: f32,
    pub status: i32,
    pub label_dist: f32,
    pub last_se: f32,
    pub predicted_val: f32,
    pub sum_ci_vi_ui: f64,
    pub sum_ci_vi: f64,
    pub target_dist: f32,
}

/// `GCA_MORPH`: the grid of nodes and the morph's header.
#[derive(Debug)]
pub struct Gcam {
    pub width: i32,
    pub height: i32,
    pub depth: i32,
    /// The nodes, `x` slowest, then `y`, then `z`.
    pub nodes: Vec<GcamNode>,
    pub neg: i32,
    pub exp_k: f64,
    pub spacing: i32,
    /// `mri_xind`, `mri_yind`, `mri_zind`: the inverse's index volumes ([`gcam_invert`]).
    pub mri_xind: Option<Mri>,
    pub mri_yind: Option<Mri>,
    pub mri_zind: Option<Mri>,
    /// `image`: the volume the morph maps to; `atlas`: the volume it is defined over.
    pub image: VolGeom,
    pub atlas: VolGeom,
    pub ninputs: i32,
    pub mtype: i32,
    pub status: i32,
    pub m_affine: Option<Matrix>,
    pub det: f64,
    /// The port's own: the geometries came from a warp volume that has none (their sizes are
    /// memory contents in FreeSurfer: FS-855).
    pub geom_unset: bool,
}

impl Gcam {
    fn idx(&self, x: i32, y: i32, z: i32) -> usize {
        ((x as usize * self.height as usize) + y as usize) * self.depth as usize + z as usize
    }

    /// The node at `(x, y, z)` (inside the grid).
    pub fn node(&self, x: i32, y: i32, z: i32) -> &GcamNode {
        &self.nodes[self.idx(x, y, z)]
    }

    /// The node at `(x, y, z)` (inside the grid), to change.
    pub fn node_mut(&mut self, x: i32, y: i32, z: i32) -> &mut GcamNode {
        let i = self.idx(x, y, z);
        &mut self.nodes[i]
    }

    fn count(&self) -> usize {
        if self.width <= 0 || self.height <= 0 || self.depth <= 0 {
            0
        } else {
            self.width as usize * self.height as usize * self.depth as usize
        }
    }
}

/// Where `GCAMalloc`'s allocations fail on a machine of [`MACHINE_BYTES`] (FS-851) or for a
/// negative count (`calloc` of a size that wraps around): its message, or `None` when all succeed.
/// The node pointers' table (`width` pointers), then for each `x` a row table (`height` pointers)
/// and its `height` rows of `depth` nodes, in that order, each written as it is made.
fn alloc_failure(width: i32, height: i32, depth: i32) -> Option<String> {
    if width < 0 {
        return Some("GCAMalloc: could not allocate nodes".to_owned());
    }
    let mut total = u64::from(width.unsigned_abs()) * SIZEOF_PTR;
    if total > MACHINE_BYTES {
        return Some("GCAMalloc: could not allocate nodes".to_owned());
    }
    if width == 0 {
        return None;
    }
    if height < 0 {
        return Some("GCAMalloc: could not allocate 0th **".to_owned());
    }
    let row_table = u64::from(height.unsigned_abs()) * SIZEOF_PTR;
    if height > 0 && depth < 0 {
        return Some("GCAMalloc: could not allocate 0,0th *".to_owned());
    }
    let row = u64::from(depth.max(0).unsigned_abs()) * SIZEOF_NODE;
    let per_x = row_table + u64::from(height.unsigned_abs()) * row;
    // the first x whose row table or rows pass the machine's memory
    let room = MACHINE_BYTES - total;
    let full = room.checked_div(per_x).unwrap_or(u64::MAX);
    if full >= u64::from(width.unsigned_abs()) {
        return None;
    }
    let x = full;
    total += full * per_x;
    if total + row_table > MACHINE_BYTES {
        return Some(format!("GCAMalloc: could not allocate {x}th **"));
    }
    total += row_table;
    let y = (MACHINE_BYTES - total) / row.max(1);
    Some(format!("GCAMalloc: could not allocate {x},{y}th *"))
}

/// `GCAMalloc(width, height, depth)`: a morph of `width x height x depth` nodes, each at its own
/// coordinates (`origx = x`, `x = x`, ...), every other field 0; spacing 1, type `GCAM_VOX`, both
/// geometries `initVolGeom`'s. An allocation that fails ends the program (`GCAMalloc: could not
/// allocate nodes`, `... <x>th **`, `... <x>,<y>th *`); FS-851: past [`MACHINE_BYTES`] of nodes the
/// port answers as a machine that refuses them, in both modes.
pub fn gcam_alloc(
    g: &mut Globals,
    err: &mut dyn Write,
    width: i32,
    height: i32,
    depth: i32,
) -> Result<Gcam, Fatal> {
    if let Some(msg) = alloc_failure(width, height, depth) {
        let need = u64::from(width.max(0).unsigned_abs())
            * u64::from(height.max(0).unsigned_abs())
            * u64::from(depth.max(0).unsigned_abs())
            * SIZEOF_NODE;
        if width >= 0 && height >= 0 && depth >= 0 && need > MACHINE_BYTES / 2 {
            g.notices.add(
                &FS_851,
                format!(
                    "GCAMalloc of {width} x {height} x {depth} nodes ({need} bytes): refused past 16 GiB, as a machine of that memory does (FreeSurfer's outcome depends on the machine)"
                ),
            );
        }
        g.errno = larmorx_freesurfer_core::alloc::ENOMEM;
        log::error_exit(g, err, ERROR_NOMEMORY, &msg)?;
    }
    let mut gcam = Gcam {
        width,
        height,
        depth,
        nodes: Vec::new(),
        neg: 0,
        exp_k: 0.0,
        spacing: 1,
        mri_xind: None,
        mri_yind: None,
        mri_zind: None,
        image: VolGeom::default(),
        atlas: VolGeom::default(),
        ninputs: 0,
        mtype: GCAM_VOX,
        status: 0,
        m_affine: None,
        det: 0.0,
        geom_unset: false,
    };
    let n = gcam.count();
    let mut nodes = Vec::with_capacity(n);
    if n > 0 {
        for x in 0..width {
            for y in 0..height {
                for z in 0..depth {
                    let (fx, fy, fz) = (f64::from(x), f64::from(y), f64::from(z));
                    nodes.push(GcamNode {
                        origx: fx,
                        origy: fy,
                        origz: fz,
                        x: fx,
                        y: fy,
                        z: fz,
                        ..GcamNode::default()
                    });
                }
            }
        }
    }
    gcam.nodes = nodes;
    init_vol_geom(&mut gcam.image);
    init_vol_geom(&mut gcam.atlas);
    Ok(gcam)
}

/// A compressed file whose data zlib could not inflate (its error state, which every later read
/// answers with -1 without reaching the end).
fn damaged(fp: &mut ZnzFile) -> bool {
    fp.gz().is_some_and(|gz| gz.gzread(&mut []) < 0)
}

/// `__m3zRead(fname)`: the morph of an `.m3z` (compressed: a name holding `.m3z`) or `.m3d` file.
/// A big-endian float version (1.0, else `GCAMread(<name>): invalid version # <v>`), the grid's
/// sizes and spacing (`int`s), `exp_k` (a float); per node (x slowest) `origx`, `origy`, `origz`,
/// `x`, `y`, `z` (floats) and `xn`, `yn`, `zn` (`int`s); a node whose six positions are all within
/// `FLT_EPSILON` of 0 is `GCAM_POSITION_INVALID`, else one on the grid's border is
/// `GCAM_AREA_INVALID`. `det` 1, both geometries not valid. Then, unless `FS_SKIP_TAGS` is set,
/// tags until a tag 0 or the end of the file: `TAG_GCAMORPH_LABELS` (an `int` per node; status
/// `GCAM_LABELED`), `TAG_GCAMORPH_GEOM` (the image's and the atlas's geometries), `TAG_GCAMORPH_TYPE`
/// (an `int`), `TAG_MGH_XFORM` (without a length; the affine matrix's tag and text after it,
/// `det` its determinant); other tags (with a length, which is not skipped) are passed over.
/// FS-854: a compressed file damaged after its nodes is refused by default (FreeSurfer's tag loop
/// never ends, or ends as memory contents decide); `reproduce` ends the tags there.
fn m3z_read(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    fname: &str,
) -> Result<Option<Gcam>, Fatal> {
    let gzipped = fname.contains(".m3z");
    let path = utils::resolve(dir, fname);
    let folder = path.is_dir();
    let opened = if folder {
        None
    } else {
        ZnzFile::open(path.as_os_str().as_encoded_bytes(), b"rb", gzipped)
    };
    let Some(mut fp) = opened else {
        if folder {
            // Linux opens a folder and its first read fails (EISDIR): the version is memory
            // contents (FS-167), 0 here
            g.errno = 21;
            log::error_printf(g, err, ERROR_BADFILE, "freadFloat: fread failed");
            fio::memory(g, &larmorx_core::defects::FS_167, "znzreadFloat", 0, 4);
            log::error_printf(
                g,
                err,
                ERROR_BADFILE,
                &format!(
                    "GCAMread({fname}): invalid version # {}\n",
                    fmt_f(0.0, 2, 3)
                ),
            );
            return Ok(None);
        }
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("GCAMread({fname}): could not open file"),
        );
        return Ok(None);
    };
    let version = fio::znz_read_float(g, err, &mut fp);
    if f64::from(version) != GCAM_VERSION {
        log::error_printf(
            g,
            err,
            ERROR_BADFILE,
            &format!(
                "GCAMread({fname}): invalid version # {}\n",
                fmt_f(f64::from(version), 2, 3)
            ),
        );
        return Ok(None);
    }
    let width = fio::znz_read_int(g, err, &mut fp);
    let height = fio::znz_read_int(g, err, &mut fp);
    let depth = fio::znz_read_int(g, err, &mut fp);
    let mut gcam = gcam_alloc(g, err, width, height, depth)?;
    gcam.spacing = fio::znz_read_int(g, err, &mut fp);
    gcam.exp_k = f64::from(fio::znz_read_float(g, err, &mut fp));
    let n = gcam.count();
    let (w, h, d) = (width.max(0), height.max(0), depth.max(0));
    if n > 0 {
        let mut i = 0;
        for x in 0..w {
            for y in 0..h {
                for z in 0..d {
                    let mut f = || f64::from(fio::znz_read_float(g, err, &mut fp));
                    let (ox, oy, oz) = (f(), f(), f());
                    let (px, py, pz) = (f(), f(), f());
                    let xn = fio::znz_read_int(g, err, &mut fp);
                    let yn = fio::znz_read_int(g, err, &mut fp);
                    let zn = fio::znz_read_int(g, err, &mut fp);
                    let node = &mut gcam.nodes[i];
                    i += 1;
                    (node.origx, node.origy, node.origz) = (ox, oy, oz);
                    (node.x, node.y, node.z) = (px, py, pz);
                    (node.xn, node.yn, node.zn) = (xn, yn, zn);
                    node.invalid = if [ox, oy, oz, px, py, pz].iter().all(|&v| fzero(v)) {
                        GCAM_POSITION_INVALID
                    } else if x == 0
                        || x == width - 1
                        || y == 0
                        || y == height - 1
                        || z == 0
                        || z == depth - 1
                    {
                        GCAM_AREA_INVALID
                    } else {
                        GCAM_VALID
                    };
                }
            }
        }
    }
    gcam.det = 1.0;
    gcam.image.valid = 0;
    gcam.atlas.valid = 0;
    let show = g.gdiag & DIAG_SHOW != 0 && verbose_on(g);
    while env.get("FS_SKIP_TAGS").is_none() {
        if damaged(&mut fp) {
            g.notices.add(
                &FS_854,
                "GCAMread: the compressed morph is damaged after its nodes: FreeSurfer's reading of its tags never ends, or ends as memory contents decide (reproduce: ended here)",
            );
            if g.notices.reproduce() {
                // the tags end here, as when memory gives FreeSurfer a tag 0
                break;
            }
            g.errno = 0;
            log::error_printf(
                g,
                err,
                ERROR_BADFILE,
                &format!("GCAMread({fname}): the compressed data is damaged"),
            );
            return Ok(None);
        }
        let mut len = 0i64;
        let tag = tags::znz_tag_read_start(g, err, &mut fp, &mut len, tags::TAG_MGH_XFORM);
        if tag == 0 {
            break;
        }
        match tag {
            tags::TAG_GCAMORPH_LABELS => {
                if show {
                    let _ = out.write_all(b"reading labels out of gcam file...\n");
                }
                gcam.status = GCAM_LABELED;
                for i in 0..n {
                    gcam.nodes[i].label = fio::znz_read_int(g, err, &mut fp);
                }
            }
            tags::TAG_GCAMORPH_GEOM => {
                larmorx_freesurfer_io::fstagsio::vol_geom_read(
                    g,
                    err,
                    &mut fp,
                    &mut gcam.image,
                    false,
                    true,
                );
                larmorx_freesurfer_io::fstagsio::vol_geom_read(
                    g,
                    err,
                    &mut fp,
                    &mut gcam.atlas,
                    false,
                    true,
                );
                if show {
                    let _ = err.write_all(
                        b"GCAMORPH_GEOM tag found.  Reading src and dst information.\nsrc geometry:\n",
                    );
                    let _ = err.write_all(&vol_geom_text(&gcam.image));
                    let _ = err.write_all(b"dst geometry:\n");
                    let _ = err.write_all(&vol_geom_text(&gcam.atlas));
                }
            }
            tags::TAG_GCAMORPH_TYPE => {
                gcam.mtype = fio::znz_read_int(g, err, &mut fp);
                if show {
                    let t = if gcam.mtype == GCAM_VOX { "vox" } else { "ras" };
                    let _ = writeln!(out, "gcam->type = {t}");
                }
            }
            tags::TAG_MGH_XFORM => {
                let m = tags::znz_read_matrix(g, err, &mut fp);
                gcam.det = f64::from(matrix_determinant(g, err, &m));
                gcam.m_affine = Some(m);
            }
            _ => {}
        }
    }
    Ok(Some(gcam))
}

/// `Warpfield::read(fname)`: the morph of an MGH or NIfTI warp volume (another format:
/// `[ERROR] Warpfield::read(): <name> is not in mgz/nii format`), read with `MRIread` (`[ERROR]
/// Warpfield::read() failed reading <name>` when it fails) and refused unless its version is the
/// warp's (`(MGZ_INTENT_WARPMAP << 8) | MGH_VERSION`: `[ERROR] <name> is not mgz warp file`). A
/// node per voxel at its own coordinates (`origx = c`, `xn = c`, ...; every node valid), `det` 1
/// or the affine's determinant (printed with `[DEBUG]` lines and the matrix twice), spacing and
/// `exp_k` from the volume's morph metadata, type `GCAM_VOX`, the geometries the volume's; status
/// `GCAM_LABELED` and the labels when it has them; the positions from the three frames by the
/// warp's format: absolute or displaced voxels (`x = v` or `x = v + c`), absolute or displaced RAS
/// (`v`, or `v` plus the atlas voxel's RAS, through the image's RAS-to-voxel matrix with shears;
/// the products in `double`, the sums in `float`); an unknown format leaves the nodes where they
/// are.
fn warpfield_read(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    fname: &str,
) -> Result<Option<Gcam>, Fatal> {
    let ty = identify::mri_identify(g, dir, env, out, err, fname)?;
    if ty != identify::MRI_MGH_FILE && ty != identify::NII_FILE {
        let _ = writeln!(
            out,
            "[ERROR] Warpfield::read(): {fname} is not in mgz/nii format"
        );
        return Ok(None);
    }
    let mgz_version = ((MGZ_INTENT_WARPMAP & 0xffff) << 8) | MGH_VERSION;
    let Some(warp) = mriio::mri_read_volume(g, out, err, env, dir, fname)? else {
        let _ = writeln!(out, "[ERROR] Warpfield::read() failed reading {fname}");
        return Ok(None);
    };
    if warp.version != mgz_version {
        let _ = writeln!(out, "[ERROR] {fname} is not mgz warp file");
        return Ok(None);
    }
    let (w, h, d) = (warp.vg.width, warp.vg.height, warp.vg.depth);
    let mut gcam = gcam_alloc(g, err, w, h, d)?;
    gcam.det = 1.0;
    gcam.spacing = warp.gcamorph_spacing;
    gcam.exp_k = warp.gcamorph_exp_k;
    gcam.mtype = GCAM_VOX;
    gcam.image.assign(g, &warp.gcamorph_image_vg);
    gcam.atlas.assign(g, &warp.gcamorph_atlas_vg);
    // a geometry the volume's tags never wrote: its validity and sizes are memory contents in
    // FreeSurfer (0 here)
    gcam.geom_unset = warp.gcamorph_image_vg == VolGeom::default()
        || warp.gcamorph_atlas_vg == VolGeom::default();
    let src_vox2ras = xfm_crs2xyz(g, out, err, &gcam.image, 0, true)?;
    let src_ras2vox = matrix_inverse(g, out, err, Some(&src_vox2ras), None)?.into_new();
    let dst_vox2ras = xfm_crs2xyz(g, out, err, &gcam.atlas, 0, true)?;
    // FreeSurfer computes the atlas's RAS-to-voxel matrix too, unused
    let _ = matrix_inverse(g, out, err, Some(&dst_vox2ras), None)?;
    if let Some(a) = &warp.gcamorph_affine {
        let _ = writeln!(
            out,
            "[DEBUG] Warpfield::read() __warpmap->gcamorphAffine (spacing={}, exp-k={}):",
            warp.gcamorph_spacing,
            fmt_f(warp.gcamorph_exp_k, 0, 2)
        );
        print_matrix(g, out, err, a);
        let m = matrix_copy(g, out, err, Some(a), None)?
            .into_new()
            .expect("a copy");
        gcam.det = f64::from(matrix_determinant(g, err, &m));
        let _ = writeln!(
            out,
            "[DEBUG] Warpfield::read() gcam->m_affine (spacing={}, exp-k={}, det={}):",
            gcam.spacing,
            fmt_f(gcam.exp_k, 0, 2),
            fmt_f(gcam.det, 0, 2)
        );
        print_matrix(g, out, err, &m);
        gcam.m_affine = Some(m);
    }
    if warp.gcamorph_label.is_some() {
        let _ = out.write_all(b"[DEBUG] Warpfield::read() gcam->status = GCAM_LABELED\n");
        gcam.status = GCAM_LABELED;
    }
    let fmt = warp.warp_field_format;
    if gcam.geom_unset && (fmt == WARPFIELD_DTFMT_ABS_RAS || fmt == WARPFIELD_DTFMT_DISP_RAS) {
        unset_geometry(g, "its RAS positions");
    }
    let mut image_crs = Matrix::real(4, 1);
    let mut image_ras = Matrix::real(4, 1);
    let mut atlas_crs0 = Matrix::real(4, 1);
    let mut atlas_ras0 = Matrix::real(4, 1);
    for c in 0..w.max(0) {
        for r in 0..h.max(0) {
            for s in 0..d.max(0) {
                let label = warp
                    .gcamorph_label
                    .as_ref()
                    .map(|l| l[((c as usize * h as usize) + r as usize) * d as usize + s as usize]);
                let v = |g: &mut Globals, err: &mut dyn Write, f: i32| {
                    mri_get_vox_val(g, err, &warp, c, r, s, f, 0)
                };
                {
                    let n = gcam.node_mut(c, r, s);
                    n.origx = f64::from(c as f32);
                    n.origy = f64::from(r as f32);
                    n.origz = f64::from(s as f32);
                    n.xn = c;
                    n.yn = r;
                    n.zn = s;
                    if let Some(l) = label {
                        n.label = l;
                    }
                }
                if fmt == WARPFIELD_DTFMT_ABS_CRS || fmt == WARPFIELD_DTFMT_DISP_CRS {
                    let (a, b, cc) = (v(g, err, 0)?, v(g, err, 1)?, v(g, err, 2)?);
                    let n = gcam.node_mut(c, r, s);
                    if fmt == WARPFIELD_DTFMT_ABS_CRS {
                        (n.x, n.y, n.z) = (f64::from(a), f64::from(b), f64::from(cc));
                    } else {
                        n.x = f64::from(a) + n.origx;
                        n.y = f64::from(b) + n.origy;
                        n.z = f64::from(cc) + n.origz;
                    }
                } else if fmt == WARPFIELD_DTFMT_ABS_RAS || fmt == WARPFIELD_DTFMT_DISP_RAS {
                    if fmt == WARPFIELD_DTFMT_ABS_RAS {
                        image_ras.set_relt(1, 1, v(g, err, 0)?);
                        image_ras.set_relt(2, 1, v(g, err, 1)?);
                        image_ras.set_relt(3, 1, v(g, err, 2)?);
                        image_ras.set_relt(4, 1, 1.0);
                    } else {
                        atlas_crs0.set_relt(1, 1, c as f32);
                        atlas_crs0.set_relt(2, 1, r as f32);
                        atlas_crs0.set_relt(3, 1, s as f32);
                        atlas_crs0.set_relt(4, 1, 1.0);
                        matrix_multiply_d(
                            g,
                            out,
                            err,
                            Some(&dst_vox2ras),
                            Some(&atlas_crs0),
                            Some(&mut atlas_ras0),
                        )?;
                        for k in 1..=3 {
                            let val = v(g, err, k - 1)?;
                            image_ras.set_relt(k, 1, sse::add(val, atlas_ras0.relt(k, 1)));
                        }
                        image_ras.set_relt(4, 1, 1.0);
                    }
                    matrix_multiply_d(
                        g,
                        out,
                        err,
                        src_ras2vox.as_ref(),
                        Some(&image_ras),
                        Some(&mut image_crs),
                    )?;
                    let n = gcam.node_mut(c, r, s);
                    n.x = f64::from(image_crs.relt(1, 1));
                    n.y = f64::from(image_crs.relt(2, 1));
                    n.z = f64::from(image_crs.relt(3, 1));
                }
            }
        }
    }
    Ok(Some(gcam))
}

/// FS-855: a morph read from a warp volume without its geometries uses their sizes.
fn unset_geometry(g: &mut Globals, what: &str) {
    g.notices.add(
        &FS_855,
        format!(
            "a GCA morph read from a warp volume without its geometries: {what} use their sizes, which are memory contents in FreeSurfer (0 here)"
        ),
    );
}

/// `MatrixPrint(stdout, m)` (precision 5).
fn print_matrix(g: &mut Globals, out: &mut dyn Write, err: &mut dyn Write, m: &Matrix) {
    let mut buf = Vec::new();
    matrix_print(g, Some(&mut buf), out, err, Some(m), 5);
    let _ = out.write_all(&buf);
}

/// `spacing * spacing * spacing` in C's `int` (FS-853: by default the exact cube).
fn spacing_cube(g: &mut Globals, spacing: i32) -> f64 {
    let exact = f64::from(spacing).powi(3);
    let wrapped = spacing.wrapping_mul(spacing).wrapping_mul(spacing);
    if f64::from(wrapped) != exact {
        g.notices.add(
            &FS_853,
            format!(
                "GCAMread: the cube of the spacing {spacing} passes 32 bits (FreeSurfer's int gives {wrapped})"
            ),
        );
        if g.notices.reproduce() {
            return f64::from(wrapped);
        }
    }
    exact
}

/// `GCAMread(fname)`: a morph from a file `fopen` can read (else `ERROR: cannot find or read
/// <name>`): an `.m3z`/`.m3d` file ([`m3z_read`]), or an MGH or NIfTI warp volume
/// ([`warpfield_read`]; its spacing 0 taken as 1); another format gives nothing. A morph with a
/// positive `det` gets `orig_area`, `orig_area1` and `orig_area2` of every node set to `spacing^3 /
/// det` (`setting orig areas to linear transform determinant scaled <a>`); then the original
/// properties ([`gcam_compute_original_properties`]), the metric properties
/// ([`gcam_compute_metric_properties`]) and the original positions saved.
pub fn gcam_read(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    fname: &str,
) -> Result<Option<Gcam>, Fatal> {
    if fio::fio_file_exists_readable(g, dir, fname) == 0 {
        let _ = writeln!(out, "ERROR: cannot find or read {fname}");
        return Ok(None);
    }
    let ty = identify::mri_identify(g, dir, env, out, err, fname)?;
    let gcam = if ty == identify::MGH_MORPH {
        m3z_read(g, out, err, env, dir, fname)?
    } else if ty == identify::MRI_MGH_FILE || ty == identify::NII_FILE {
        let mut r = warpfield_read(g, out, err, env, dir, fname)?;
        if let Some(gc) = &mut r
            && gc.spacing == 0
        {
            gc.spacing = 1;
        }
        r
    } else {
        None
    };
    let Some(mut gcam) = gcam else {
        return Ok(None);
    };
    if gcam.det > 0.0 {
        let area = spacing_cube(g, gcam.spacing) / gcam.det;
        let _ = writeln!(
            out,
            "setting orig areas to linear transform determinant scaled {}",
            fmt_f(area, 2, 2)
        );
        let a = area as f32;
        for n in &mut gcam.nodes {
            n.orig_area = a;
            n.orig_area1 = a;
            n.orig_area2 = a;
        }
    }
    gcam_compute_original_properties(g, out, err, env, &mut gcam)?;
    gcam_compute_metric_properties(g, out, err, env, &mut gcam)?;
    gcam_copy_node_positions(
        g,
        err,
        &mut gcam,
        ORIGINAL_POSITIONS,
        SAVED_ORIGINAL_POSITIONS,
    );
    Ok(Some(gcam))
}

/// `GCAMcomputeOriginalProperties(gcam)`: the current positions saved, the original ones made
/// current, the metric properties computed, then computed again and stored
/// ([`gcam_store_metric_properties`]), the saved positions made current again and the metric
/// properties computed again.
pub fn gcam_compute_original_properties(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    gcam: &mut Gcam,
) -> Result<i32, Fatal> {
    gcam_copy_node_positions(g, err, gcam, CURRENT_POSITIONS, SAVED_POSITIONS);
    gcam_copy_node_positions(g, err, gcam, ORIGINAL_POSITIONS, CURRENT_POSITIONS);
    gcam_compute_metric_properties(g, out, err, env, gcam)?;
    gcam_store_metric_properties(g, out, err, env, gcam)?;
    gcam_copy_node_positions(g, err, gcam, SAVED_POSITIONS, CURRENT_POSITIONS);
    gcam_compute_metric_properties(g, out, err, env, gcam)?;
    Ok(NO_ERROR)
}

/// `GCAMstoreMetricProperties(gcam)`: the metric properties computed; where `det` is within
/// `FLT_EPSILON` of 0 each node's original areas become its areas; a valid node whose original
/// area is (within `FLT_EPSILON` of) 0 or negative becomes `GCAM_AREA_INVALID`.
pub fn gcam_store_metric_properties(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    gcam: &mut Gcam,
) -> Result<i32, Fatal> {
    gcam_compute_metric_properties(g, out, err, env, gcam)?;
    let no_det = fzero(gcam.det);
    for n in &mut gcam.nodes {
        if no_det {
            n.orig_area = n.area;
            n.orig_area1 = n.area1;
            n.orig_area2 = n.area2;
        }
        let oa = f64::from(n.orig_area);
        if (fzero(oa) || n.orig_area < 0.0) && n.invalid == 0 {
            n.invalid = GCAM_AREA_INVALID;
        }
    }
    Ok(NO_ERROR)
}

/// `GCAMN_SUB(a, b, v)` then the vector's `float`s: `a - b` in `double` per axis, stored as `float`.
fn sub3(a: &GcamNode, b: &GcamNode) -> [f32; 3] {
    [(a.x - b.x) as f32, (a.y - b.y) as f32, (a.z - b.z) as f32]
}

/// `VectorTripleProduct(v1, v2, v3)`: `(v1 x v2) . v3` in `float`, summed as FreeSurfer sums it.
fn triple(v1: [f32; 3], v2: [f32; 3], v3: [f32; 3]) -> f32 {
    let [x1, y1, z1] = v1;
    let [x2, y2, z2] = v2;
    let [x3, y3, z3] = v3;
    let cross = |a: f32, b: f32, c: f32, d: f32| sse::sub(sse::mul(a, b), sse::mul(c, d));
    let mut total = sse::mul(x3, cross(y1, z2, z1, y2));
    total = sse::add(total, sse::mul(y3, cross(z1, x2, x1, z2)));
    sse::add(total, sse::mul(z3, cross(x1, y2, y1, x2)))
}

/// `cma_label_to_name(label)`: the label's name (`utils/gca.cpp`'s list; the first equal one), or
/// the empty name FreeSurfer's never-written buffer holds.
pub fn cma_label_to_name(label: i32) -> &'static str {
    NAMES
        .iter()
        .find(|(l, _)| *l == label)
        .map_or("", |(_, n)| n)
}

/// `gcamComputeMetricProperties(gcam)`: each node's volume. A node whose position is invalid is
/// skipped. Otherwise `area` starts at 0; towards the next nodes (when they exist and none of the
/// three has an invalid position) `area1` is the triple product `(vj x vk) . vi` of the
/// differences to the nodes after it in y, z and x (`area1` 0 at the far border); towards the
/// previous ones (likewise) `area2` is the triple product of the differences from the previous
/// node in x to it, and to the next nodes in y and z... as FreeSurfer writes them (`area2` 0 at the
/// near border); each one computed is added to `area` (in `double`, stored as `float`), which is
/// then their mean (`area / (float) num`); a node with neither becomes `GCAM_AREA_INVALID` with
/// `area` 0. A valid node with a parallelepiped of volume 0 or less and a positive original area
/// counts as negative (`neg`); with `SHOW_NEG` set FreeSurfer prints `node (<x>, <y>, <z>), label
/// <name> (<label>) - NEGATIVE!` for each of them (FS-852: in the order x, y, z, as on one thread).
/// The first call in a process on a grid narrower than 4 prints `<file>:3513 width:<w> too small
/// for parallelism` on standard error.
pub fn gcam_compute_metric_properties(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    gcam: &mut Gcam,
) -> Result<i32, Fatal> {
    let (width, height, depth) = (gcam.width, gcam.height, gcam.depth);
    gcam.neg = 0;
    if width < 4 && !g.gcam_metric_width_reported {
        g.gcam_metric_width_reported = true;
        let _ = writeln!(
            err,
            "{BUILD_ROOT}/utils/gcamorph.cpp:3513 width:{width} too small for parallelism"
        );
    }
    let show_neg = env.get("SHOW_NEG").is_some();
    let mut neg_x: Option<i32> = None;
    let mut neg_columns = false;
    let mut neg = 0;
    for i in 0..width.max(0) {
        for j in 0..height.max(0) {
            for k in 0..depth.max(0) {
                let me = gcam.idx(i, j, k);
                if gcam.nodes[me].invalid == GCAM_POSITION_INVALID {
                    continue;
                }
                let (mut bad, mut num) = (false, 0);
                let mut area = 0f32;
                let mut area1 = None;
                let mut area2 = None;
                if i < width - 1 && j < height - 1 && k < depth - 1 {
                    let ni = &gcam.nodes[gcam.idx(i + 1, j, k)];
                    let nj = &gcam.nodes[gcam.idx(i, j + 1, k)];
                    let nk = &gcam.nodes[gcam.idx(i, j, k + 1)];
                    if ni.invalid != GCAM_POSITION_INVALID
                        && nj.invalid != GCAM_POSITION_INVALID
                        && nk.invalid != GCAM_POSITION_INVALID
                    {
                        num += 1;
                        let n = &gcam.nodes[me];
                        let a1 = triple(sub3(nj, n), sub3(nk, n), sub3(ni, n));
                        if a1 <= 0.0 {
                            bad = true;
                        }
                        area1 = Some(a1);
                        area = (f64::from(area) + f64::from(a1)) as f32;
                    }
                } else {
                    area1 = Some(0.0);
                }
                if i > 0 && j > 0 && k > 0 {
                    let ni = &gcam.nodes[gcam.idx(i - 1, j, k)];
                    let nj = &gcam.nodes[gcam.idx(i, j - 1, k)];
                    let nk = &gcam.nodes[gcam.idx(i, j, k - 1)];
                    if ni.invalid != GCAM_POSITION_INVALID
                        && nj.invalid != GCAM_POSITION_INVALID
                        && nk.invalid != GCAM_POSITION_INVALID
                    {
                        num += 1;
                        let n = &gcam.nodes[me];
                        let a2 = triple(sub3(nj, n), sub3(nk, n), sub3(n, ni));
                        area2 = Some(a2);
                        if a2 <= 0.0 {
                            bad = true;
                        }
                        area = (f64::from(area) + f64::from(a2)) as f32;
                    }
                } else {
                    area2 = Some(0.0);
                }
                let n = &mut gcam.nodes[me];
                if let Some(a) = area1 {
                    n.area1 = a;
                }
                if let Some(a) = area2 {
                    n.area2 = a;
                }
                if num > 0 {
                    n.area = sse::div(area, num as f32);
                } else {
                    n.invalid = GCAM_AREA_INVALID;
                    n.area = 0.0;
                }
                if n.invalid == GCAM_VALID && bad && n.orig_area > 0.0 {
                    if show_neg {
                        let label = n.label;
                        let _ = writeln!(
                            out,
                            "node ({i}, {j}, {k}), label {} ({label}) - NEGATIVE!",
                            cma_label_to_name(label)
                        );
                        match neg_x {
                            Some(x) if x != i => neg_columns = true,
                            _ => neg_x = Some(i),
                        }
                    }
                    neg += 1;
                }
            }
        }
    }
    gcam.neg = neg;
    if neg_columns {
        g.notices.add(
            &FS_852,
            "gcamComputeMetricProperties with SHOW_NEG: the negative nodes lie in more than one column of x (FreeSurfer's lines come in its threads' order; here x, y, z)",
        );
    }
    Ok(NO_ERROR)
}

/// `GCAMcopyNodePositions(gcam, from, to)`: one set of each node's positions copied over another
/// (original, saved, current, saved original, the second saved set); a pair FreeSurfer does not
/// copy gives `GCAMcopyNodePositions: unsupported from <f>` or `... to <t>` and `ERROR_BADPARM`
/// (at the first node: nothing copied).
pub fn gcam_copy_node_positions(
    g: &mut Globals,
    err: &mut dyn Write,
    gcam: &mut Gcam,
    from: i32,
    to: i32,
) -> i32 {
    type Get = fn(&GcamNode) -> [f64; 3];
    type Set = fn(&mut GcamNode, [f64; 3]);
    let orig: (Get, Set) = (
        |n| [n.origx, n.origy, n.origz],
        |n, v| [n.origx, n.origy, n.origz] = v,
    );
    let saved: (Get, Set) = (|n| [n.xs, n.ys, n.zs], |n, v| [n.xs, n.ys, n.zs] = v);
    let current: (Get, Set) = (|n| [n.x, n.y, n.z], |n, v| [n.x, n.y, n.z] = v);
    let saved_orig: (Get, Set) = (
        |n| [n.saved_origx, n.saved_origy, n.saved_origz],
        |n, v| [n.saved_origx, n.saved_origy, n.saved_origz] = v,
    );
    let saved2: (Get, Set) = (|n| [n.xs2, n.ys2, n.zs2], |n, v| [n.xs2, n.ys2, n.zs2] = v);
    let pick = |p: i32| match p {
        ORIGINAL_POSITIONS => Some(orig),
        SAVED_POSITIONS => Some(saved),
        CURRENT_POSITIONS => Some(current),
        SAVED_ORIGINAL_POSITIONS => Some(saved_orig),
        SAVED2_POSITIONS => Some(saved2),
        _ => None,
    };
    let allowed = match from {
        ORIGINAL_POSITIONS => [
            SAVED_ORIGINAL_POSITIONS,
            SAVED_POSITIONS,
            CURRENT_POSITIONS,
            -1,
        ],
        SAVED_ORIGINAL_POSITIONS => [SAVED_POSITIONS, CURRENT_POSITIONS, ORIGINAL_POSITIONS, -1],
        SAVED_POSITIONS => [
            ORIGINAL_POSITIONS,
            CURRENT_POSITIONS,
            SAVED_ORIGINAL_POSITIONS,
            -1,
        ],
        SAVED2_POSITIONS => [
            ORIGINAL_POSITIONS,
            CURRENT_POSITIONS,
            SAVED_ORIGINAL_POSITIONS,
            -1,
        ],
        CURRENT_POSITIONS => [
            ORIGINAL_POSITIONS,
            SAVED_POSITIONS,
            SAVED2_POSITIONS,
            SAVED_ORIGINAL_POSITIONS,
        ],
        _ => {
            if gcam.count() > 0 {
                log::error_printf(
                    g,
                    err,
                    ERROR_BADPARM,
                    &format!("GCAMcopyNodePositions: unsupported from {from}"),
                );
                return ERROR_BADPARM;
            }
            return NO_ERROR;
        }
    };
    if !allowed.contains(&to) || to < 0 {
        if gcam.count() > 0 {
            log::error_printf(
                g,
                err,
                ERROR_BADPARM,
                &format!("GCAMcopyNodePositions: unsupported to {to}"),
            );
            return ERROR_BADPARM;
        }
        return NO_ERROR;
    }
    let (get, _) = pick(from).expect("a position");
    let (_, set) = pick(to).expect("a position");
    for n in &mut gcam.nodes {
        let v = get(n);
        set(n, v);
    }
    NO_ERROR
}

/// `GCAMrasToVox(gcam, mri)`: each node's current and original positions taken from RAS to the
/// image's voxels: through the image geometry's voxel-to-RAS matrix when `mri` is `None` (as
/// FreeSurfer's code does since 2018, keeping its earlier results), else the volume's RAS-to-voxel
/// matrix; the positions as `float`s through a `float` product (`MatrixMultiply`). A node that is
/// `GCAM_AREA_INVALID` becomes valid; then the original properties, type `GCAM_VOX`, and the
/// volume's geometry as the image's when one is given. `Gdiag & DIAG_SHOW` with `DIAG_VERBOSE`
/// prints `ras 2 vox matrix:` and the matrix.
pub fn gcam_ras_to_vox(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    gcam: &mut Gcam,
    mri: Option<&Mri>,
) -> Result<i32, Fatal> {
    let m = match mri {
        None => vg_get_voxel_to_ras_xform(g, out, err, &gcam.image, 0)?,
        Some(v) => match &v.vg.r_to_i {
            Some(m) => m.clone(),
            None => {
                vol_geom::extract_r_to_i(g, out, err, &v.vg)?.unwrap_or_else(|| Matrix::real(4, 4))
            }
        },
    };
    if g.gdiag & DIAG_SHOW != 0 && verbose_on(g) {
        let _ = out.write_all(b"ras 2 vox matrix:\n");
        print_matrix(g, out, err, &m);
    }
    let mut v1 = Matrix::real(4, 1);
    v1.set_relt(4, 1, 1.0);
    let mut v2 = v1.clone();
    for i in 0..gcam.nodes.len() {
        let (x, y, z) = {
            let n = &gcam.nodes[i];
            (n.x, n.y, n.z)
        };
        v1.set_relt(1, 1, x as f32);
        v1.set_relt(2, 1, y as f32);
        v1.set_relt(3, 1, z as f32);
        matrix_multiply(g, out, err, Some(&m), Some(&v1), Some(&mut v2))?;
        {
            let n = &mut gcam.nodes[i];
            n.x = f64::from(v2.relt(1, 1));
            n.y = f64::from(v2.relt(2, 1));
            n.z = f64::from(v2.relt(3, 1));
        }
        let (ox, oy, oz) = {
            let n = &gcam.nodes[i];
            (n.origx, n.origy, n.origz)
        };
        v1.set_relt(1, 1, ox as f32);
        v1.set_relt(2, 1, oy as f32);
        v1.set_relt(3, 1, oz as f32);
        matrix_multiply(g, out, err, Some(&m), Some(&v1), Some(&mut v2))?;
        let n = &mut gcam.nodes[i];
        n.origx = f64::from(v2.relt(1, 1));
        n.origy = f64::from(v2.relt(2, 1));
        n.origz = f64::from(v2.relt(3, 1));
        if n.invalid == GCAM_AREA_INVALID {
            n.invalid = GCAM_VALID;
        }
    }
    gcam_compute_original_properties(g, out, err, env, gcam)?;
    gcam.mtype = GCAM_VOX;
    if let Some(v) = mri {
        larmorx_freesurfer_volume::vgeom::get_vol_geom(&v.vg, &mut gcam.image);
    }
    Ok(NO_ERROR)
}

/// A float volume of the morph's grid with the atlas geometry's direction cosines and centre and
/// its voxel sizes times the spacing (`MRIallocSequence` or `MRIalloc`, `MRIcopyVolGeomToMRI`,
/// `MRIsetResolution`).
fn atlas_volume(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    gcam: &Gcam,
    frames: i32,
) -> Result<Mri, Fatal> {
    let mut m = if frames == 1 {
        mri_alloc(g, out, err, gcam.width, gcam.height, gcam.depth, MRI_FLOAT)?
    } else {
        mri_alloc_sequence(
            g,
            out,
            err,
            gcam.width,
            gcam.height,
            gcam.depth,
            MRI_FLOAT,
            frames,
        )?
    };
    mri_copy_vol_geom_to_mri(&mut m, &gcam.atlas);
    let s = gcam.spacing as f32;
    mri_set_resolution(
        g,
        out,
        err,
        &mut m,
        sse::mul(gcam.atlas.xsize, s),
        sse::mul(gcam.atlas.ysize, s),
        sse::mul(gcam.atlas.zsize, s),
    )?;
    Ok(m)
}

/// `GCAMwriteWarpToMRI(gcam, NULL)`: a three-frame float volume of the morph's grid (the atlas
/// geometry, voxel sizes times the spacing) holding each node's displacement in voxels, `x -
/// origx`, `y - origy`, `z - origz` (in `double`, stored as `float`).
pub fn gcam_write_warp_to_mri(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    gcam: &Gcam,
) -> Result<Mri, Fatal> {
    let mut warp = atlas_volume(g, out, err, gcam, 3)?;
    let (w, h, d) = (warp.vg.width, warp.vg.height, warp.vg.depth);
    for x in 0..w.max(0) {
        for y in 0..h.max(0) {
            for z in 0..d.max(0) {
                let n = gcam.node(x, y, z).clone();
                mri_set_vox_val(g, err, &mut warp, x, y, z, 0, (n.x - n.origx) as f32, 0)?;
                mri_set_vox_val(g, err, &mut warp, x, y, z, 1, (n.y - n.origy) as f32, 0)?;
                mri_set_vox_val(g, err, &mut warp, x, y, z, 2, (n.z - n.origz) as f32, 0)?;
                if x == g.gx && y == g.gy && z == g.gz {
                    let f = |v: f64| fmt_f(v, 2, 1);
                    let _ = writeln!(
                        out,
                        "voxel ({}, {}, {}) --> ({}, {}, {})",
                        f(n.origx),
                        f(n.origy),
                        f(n.origz),
                        f(n.x),
                        f(n.y),
                        f(n.z)
                    );
                }
            }
        }
    }
    Ok(warp)
}

/// `MIN(a, b)` of `float`s: `a < b ? a : b`.
fn cmin(a: f32, b: f32) -> f32 {
    if a < b { a } else { b }
}

/// One node's value of `which` for [`gcam_write_mri`], or `None` for a field FreeSurfer does not
/// write.
fn node_value(libm: &Libm, n: &GcamNode, which: i32) -> Option<f32> {
    let jac = |n: &GcamNode| {
        if fzero(f64::from(n.orig_area)) {
            1.0
        } else {
            sse::div(n.area, n.orig_area)
        }
    };
    Some(match which {
        GCAM_NEG => {
            if n.invalid != GCAM_POSITION_INVALID
                && ((n.orig_area1 > 0.0 && n.area1 <= 0.0)
                    || (n.orig_area2 > 0.0 && n.area2 <= 0.0))
            {
                1.0
            } else {
                0.0
            }
        }
        GCAM_MIN_AREA => {
            if n.invalid == 0 {
                cmin(n.area1, n.area2)
            } else {
                0.0
            }
        }
        GCAM_LOG_MIN_AREA => {
            if n.invalid == 0 {
                let d = cmin(n.area1, n.area2);
                // C++'s float overloads: log10f(fabsf(d)) (the official build's object code)
                if d > 1.0 {
                    0.0
                } else if d > 0.0 {
                    -libm.log10f(d.abs())
                } else {
                    libm.log10f(d.abs())
                }
            } else {
                0.0
            }
        }
        GCAM_X_GRAD => n.dx,
        GCAM_Y_GRAD => n.dy,
        GCAM_Z_GRAD => n.dz,
        GCAM_JACOBIAN => jac(n),
        GCAM_LOG_JACOBIAN => {
            let d = jac(n);
            if d < 0.0 {
                -100.0
            } else {
                // logf (C++'s float overload)
                libm.logf(d)
            }
        }
        GCAM_AREA => n.area,
        GCAM_ORIG_AREA => n.orig_area,
        GCAM_X => n.x as f32,
        GCAM_Y => n.y as f32,
        GCAM_Z => n.z as f32,
        GCAM_ORIGX => n.origx as f32,
        GCAM_ORIGY => n.origy as f32,
        GCAM_ORIGZ => n.origz as f32,
        GCAM_LABEL => n.label as f32,
        // no node read from a file has a class
        GCAM_MEANS => 0.0,
        GCAM_STATUS => n.status as f32,
        GCAM_INVALID => f32::from(n.invalid),
        GCAM_NODEX => n.xn as f32,
        GCAM_NODEY => n.yn as f32,
        GCAM_NODEZ => n.zn as f32,
        _ => return None,
    })
}

/// `GCAMwriteMRI(gcam, NULL, which)`: a float volume of the morph's grid (the atlas geometry,
/// voxel sizes times the spacing) of one field per node: the gradient, label, original and
/// current positions, node coordinates, status, validity, areas, the Jacobian `area / orig_area`
/// (1 where `orig_area` is within `FLT_EPSILON` of 0) and its logarithm (`logf`; -100 below 0),
/// whether either parallelepiped turned negative, the smaller area and its signed `log10f`
/// (0 above 1, `-log10f(d)` in (0, 1], `log10f(|d|)` otherwise: the float functions, as the
/// official build's C++ calls the float overloads; correctly rounded by default, glibc 2.35's
/// with `reproduce`, GLIBC-07 noted where the two differ on a node's value). Another field
/// ends the program (`GCAMwriteMRI(<which>): unknown which parameter`) at the first node.
pub fn gcam_write_mri(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    gcam: &Gcam,
    which: i32,
) -> Result<Mri, Fatal> {
    let mut m = atlas_volume(g, out, err, gcam, 1)?;
    let libm = Libm::new(g.mode());
    for x in 0..gcam.width.max(0) {
        for y in 0..gcam.height.max(0) {
            for z in 0..gcam.depth.max(0) {
                let Some(d) = node_value(&libm, gcam.node(x, y, z), which) else {
                    log::error_exit(
                        g,
                        err,
                        ERROR_BADPARM,
                        &format!("GCAMwriteMRI({which}): unknown which parameter"),
                    )?;
                    return Ok(m);
                };
                mri_set_vox_val(g, err, &mut m, x, y, z, 0, d, 0)?;
            }
        }
    }
    note_glibc07(
        g,
        &libm,
        if which == GCAM_LOG_JACOBIAN {
            "logf"
        } else {
            "log10f"
        },
    );
    Ok(m)
}

/// `GCAMinvert(gcam, mri)`: the inverse's index volumes (float volumes of the image's extent with
/// `mri`'s header, or an image-sized header when `mri` is `None`; nothing when they exist): every
/// node whose position is valid adds its x, y and z (and 1, to a count volume) at its current
/// position clamped into the image ([`mri_interpolate_into_volume`]), nodes in the order z, y, x;
/// each voxel with a count is divided by it and becomes a control point (unless its count is below
/// 0.1); then each index volume is spread from the control points (`MRIbuildVoronoiDiagram`, then
/// 50 passes of `MRIsoapBubble`). The volume's extent must be the image's (`mri passed volume size
/// ( <w> <h> <d> ) is different from the one used to create M3D data ( ... )`, the program ends).
pub fn gcam_invert(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    gcam: &mut Gcam,
    mri: Option<&Mri>,
) -> Result<i32, Fatal> {
    let made;
    let mri = match mri {
        Some(m) => m,
        None => {
            let vg = &gcam.image;
            made = mri_alloc_header(g, out, err, vg.width, vg.height, vg.depth, MRI_UCHAR, 1)?;
            &made
        }
    };
    if gcam.mri_xind.is_some() {
        return Ok(NO_ERROR);
    }
    let (width, height, depth) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    if width != gcam.image.width || height != gcam.image.height || depth != gcam.image.depth {
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            &format!(
                "mri passed volume size ( {width} {height} {depth} ) is different from the one used to create M3D data ( {} {} {} )\n",
                gcam.image.width, gcam.image.height, gcam.image.depth
            ),
        )?;
    }
    let index = |g: &mut Globals, out: &mut dyn Write, err: &mut dyn Write| -> Result<Mri, Fatal> {
        let mut m = mri_alloc(g, out, err, width, height, depth, MRI_FLOAT)?;
        mri_copy_header(g, out, err, mri, Some(&mut m))?;
        Ok(m)
    };
    let mut xind = index(g, out, err)?;
    let mut yind = index(g, out, err)?;
    let mut zind = index(g, out, err)?;
    let mut counts = mri_alloc(g, out, err, width, height, depth, MRI_FLOAT)?;
    let mut ctrl = mri_alloc(g, out, err, width, height, depth, MRI_UCHAR)?;
    mri_copy_header(g, out, err, mri, Some(&mut ctrl))?;
    for z in 0..gcam.depth.max(0) {
        for y in 0..gcam.height.max(0) {
            for x in 0..gcam.width.max(0) {
                let n = gcam.node(x, y, z);
                if n.invalid == GCAM_POSITION_INVALID {
                    continue;
                }
                let (mut xf, mut yf, mut zf) = (n.x, n.y, n.z);
                if xf < 0.0 {
                    xf = 0.0;
                }
                if yf < 0.0 {
                    yf = 0.0;
                }
                if zf < 0.0 {
                    zf = 0.0;
                }
                if xf >= f64::from(width) {
                    xf = f64::from(width - 1);
                }
                if yf >= f64::from(height) {
                    yf = f64::from(height - 1);
                }
                if zf >= f64::from(depth) {
                    zf = f64::from(depth - 1);
                }
                mri_interpolate_into_volume(g, err, &mut xind, xf, yf, zf, f64::from(x));
                mri_interpolate_into_volume(g, err, &mut yind, xf, yf, zf, f64::from(y));
                mri_interpolate_into_volume(g, err, &mut zind, xf, yf, zf, f64::from(z));
                mri_interpolate_into_volume(g, err, &mut counts, xf, yf, zf, 1.0);
            }
        }
    }
    let diag_write = verbose_on(g) && g.gdiag & DIAG_WRITE != 0;
    if diag_write {
        write_diag(g, out, err, env, dir, &mut xind, "xi.mgz")?;
        write_diag(g, out, err, env, dir, &mut counts, "c.mgz")?;
    }
    for z in 0..depth.max(0) {
        for y in 0..height.max(0) {
            for x in 0..width.max(0) {
                let num = mri_get_vox_val(g, err, &counts, x, y, z, 0, 0)?;
                if num == 0.0 {
                    continue;
                }
                for v in [&mut xind, &mut yind, &mut zind] {
                    let a = mri_get_vox_val(g, err, v, x, y, z, 0, 0)?;
                    mri_set_vox_val(g, err, v, x, y, z, 0, sse::div(a, num), 0)?;
                }
                mri_set_vox_val(g, err, &mut ctrl, x, y, z, 0, CONTROL_MARKED as f32, 0)?;
                if f64::from(num) < 0.1 {
                    mri_set_vox_val(g, err, &mut ctrl, x, y, z, 0, 0.0, 0)?;
                }
            }
        }
    }
    drop(counts);
    let show = g.gdiag & DIAG_SHOW != 0 && verbose_on(g);
    for (k, v) in [&mut xind, &mut yind, &mut zind].into_iter().enumerate() {
        if show {
            let axis = ["x", "y", "z"][k];
            let _ = writeln!(out, "performing soap bubble of {axis} indices...");
        }
        // FreeSurfer passes the volume as both source and destination; the source is read only
        // at the control voxels before they are written
        let src = larmorx_freesurfer_mri::copy::mri_copy(g, out, err, v, None)?
            .into_new()
            .expect("a copy");
        mri_build_voronoi_diagram(g, out, err, &src, &mut ctrl, Some(v))?;
        drop(src);
        if k == 0 && diag_write {
            write_diag(g, out, err, env, dir, v, "xi.mgz")?;
        }
        mri_soap_bubble(g, out, err, Pair::Same(v), &ctrl, 50, 1.0)?;
        if k == 0 && diag_write {
            write_diag(g, out, err, env, dir, v, "xis.mgz")?;
        }
    }
    if g.gdiag & DIAG_WRITE != 0 && verbose_on(g) {
        write_diag(g, out, err, env, dir, &mut xind, "xi.mgz")?;
        write_diag(g, out, err, env, dir, &mut yind, "yi.mgz")?;
        write_diag(g, out, err, env, dir, &mut zind, "zi.mgz")?;
    }
    gcam.mri_xind = Some(xind);
    gcam.mri_yind = Some(yind);
    gcam.mri_zind = Some(zind);
    Ok(NO_ERROR)
}

/// `MRIwrite(mri, name)` of a diagnostic volume into the working folder.
fn write_diag(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    m: &mut Mri,
    name: &str,
) -> Result<(), Fatal> {
    mriio::mri_write(g, out, err, env, dir, m, name, None, -1)?;
    Ok(())
}

/// `GCAMmorphFieldFromAtlas(gcam, mri, which, 0, 0)` for the fields `mri_read` asks it for (the
/// node coordinates and the rest of its list): the field as an atlas volume ([`gcam_write_mri`]),
/// the inverse rebuilt ([`gcam_invert`]), and a volume like `mri` (short for labels, float
/// otherwise) whose voxel takes the atlas volume's value at its inverse's indices (`nint` of
/// each), nothing where they fall outside the atlas grid. An unsupported field: `None` after
/// `GCAMmorphFieldFromAtlas: unsupported field <which>`.
#[allow(clippy::too_many_arguments)]
pub fn gcam_morph_field_from_atlas(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    gcam: &mut Gcam,
    mri: &Mri,
    which: i32,
) -> Result<Option<Mri>, Fatal> {
    let atlas = gcam_write_mri(g, out, err, gcam, which)?;
    gcam.mri_xind = None;
    gcam.mri_yind = None;
    gcam.mri_zind = None;
    gcam_invert(g, out, err, env, dir, gcam, Some(mri))?;
    let ty = match which {
        GCAM_LABEL => larmorx_freesurfer_mri::mri::MRI_SHORT,
        GCAM_DIAG_VOL | GCAM_NODEX | GCAM_NODEY | GCAM_NODEZ | GCAM_ORIGX | GCAM_ORIGY
        | GCAM_ORIGZ | GCAM_X_GRAD | GCAM_Y_GRAD | GCAM_Z_GRAD | GCAM_JACOBIAN | GCAM_AREA
        | GCAM_ORIG_AREA | GCAM_MEANS | GCAM_COVARS => MRI_FLOAT,
        _ => {
            log::error_printf(
                g,
                err,
                ERROR_UNSUPPORTED,
                &format!("GCAMmorphFieldFromAtlas: unsupported field {which}"),
            );
            return Ok(None);
        }
    };
    let mut morphed = mri_clone_different_type(g, out, err, mri, ty)?;
    let (xi, yi, zi) = (
        gcam.mri_xind.as_ref().expect("the inverse"),
        gcam.mri_yind.as_ref().expect("the inverse"),
        gcam.mri_zind.as_ref().expect("the inverse"),
    );
    for x in 0..morphed.vg.width.max(0) {
        for y in 0..morphed.vg.height.max(0) {
            for z in 0..morphed.vg.depth.max(0) {
                let xv = mri_get_vox_val(g, err, xi, x, y, z, 0, 0)?;
                let yv = mri_get_vox_val(g, err, yi, x, y, z, 0, 0)?;
                let zv = mri_get_vox_val(g, err, zi, x, y, z, 0, 0)?;
                let xm = nint(g, f64::from(xv));
                let ym = nint(g, f64::from(yv));
                let zm = nint(g, f64::from(zv));
                if xm >= atlas.vg.width
                    || ym >= atlas.vg.height
                    || zm >= atlas.vg.depth
                    || xm < 0
                    || ym < 0
                    || zm < 0
                {
                    continue;
                }
                for _frame in 0..atlas.nframes {
                    let val = mri_get_vox_val(g, err, &atlas, xm, ym, zm, 0, 0)?;
                    mri_set_vox_val(g, err, &mut morphed, x, y, z, 0, val, 0)?;
                }
            }
        }
    }
    Ok(Some(morphed))
}

/// `mri_read`'s branch of type `MGH_MORPH` (mriio.cpp 656-692): the morph ([`gcam_read`]; none:
/// `MRIread(<name>): could not read .m3z`, `ERROR_BADPARM`, nothing), taken to voxels when its
/// type is `GCAM_RAS` ([`gcam_ras_to_vox`]); with no frame asked (`start_frame` below 0) its
/// displacements ([`gcam_write_warp_to_mri`], three frames), else `reading 'frame' # <f> from gcam
/// (see gcamorph.h for definitions)` and the field numbered `start_frame` as one frame: the node
/// coordinates (7, 8, 9) through the inverse onto a float volume of the image's geometry
/// ([`gcam_morph_field_from_atlas`]), any other field over the atlas grid ([`gcam_write_mri`]);
/// the frames then 0 to 0. Returns the volume (or `None`) and the frames `mri_read` goes on with.
#[allow(clippy::too_many_arguments)]
pub fn read_morph(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    dir: Option<&Path>,
    fname: &str,
    start_frame: i32,
    end_frame: i32,
) -> Result<(Option<Mri>, i32, i32), Fatal> {
    let Some(mut gcam) = gcam_read(g, out, err, env, dir, fname)? else {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("MRIread({fname}): could not read .m3z\n"),
        );
        return Ok((None, start_frame, end_frame));
    };
    if gcam.mtype == GCAM_RAS {
        gcam_ras_to_vox(g, out, err, env, &mut gcam, None)?;
    }
    if start_frame < 0 {
        let m = gcam_write_warp_to_mri(g, out, err, &gcam)?;
        return Ok((Some(m), start_frame, end_frame));
    }
    let _ = writeln!(
        out,
        "reading 'frame' # {start_frame} from gcam (see gcamorph.h for definitions)"
    );
    let which = start_frame;
    let m = if matches!(which, GCAM_NODEX | GCAM_NODEY | GCAM_NODEZ) {
        let im = gcam.image.clone();
        if gcam.geom_unset {
            unset_geometry(g, "the node-coordinate frames");
        }
        if im.width <= 0 || im.height <= 0 || im.depth <= 0 {
            // FreeSurfer inverts into an image volume without voxels and crashes
            g.notices.add(
                &FS_855,
                format!(
                    "the node-coordinate frames of a GCA morph whose image is {} x {} x {}: FreeSurfer crashes",
                    im.width, im.height, im.depth
                ),
            );
            log::error_exit(
                g,
                err,
                ERROR_BADPARM,
                &format!(
                    "GCAMmorphFieldFromAtlas: the image ({} x {} x {}) has no voxels (FreeSurfer crashes here)\n",
                    im.width, im.height, im.depth
                ),
            )?;
            return Ok((None, 0, 0));
        }
        let mut template = mri_alloc(g, out, err, im.width, im.height, im.depth, MRI_FLOAT)?;
        mri_copy_vol_geom_to_mri(&mut template, &im);
        mri_set_resolution(g, out, err, &mut template, im.xsize, im.ysize, im.zsize)?;
        vol_geom::reinit_cache(g, out, err, &mut template.vg)?;
        gcam_morph_field_from_atlas(g, out, err, env, dir, &mut gcam, &template, which)?
    } else {
        Some(gcam_write_mri(g, out, err, &gcam, which)?)
    };
    Ok((m, 0, 0))
}

/// Registers [`read_morph`] with fs.io, whose `mri_read` reads morphs through it.
pub fn register_io() {
    larmorx_freesurfer_io::upper::register(|u| {
        u.read_morph = Some(read_morph);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocation_failures_follow_freesurfer_s_order() {
        assert_eq!(alloc_failure(3, 4, 5), None);
        assert_eq!(alloc_failure(0, -1, -1), None);
        assert_eq!(
            alloc_failure(-1, 1, 1).as_deref(),
            Some("GCAMalloc: could not allocate nodes")
        );
        assert_eq!(
            alloc_failure(2, -3, 1).as_deref(),
            Some("GCAMalloc: could not allocate 0th **")
        );
        assert_eq!(
            alloc_failure(2, 3, -1).as_deref(),
            Some("GCAMalloc: could not allocate 0,0th *")
        );
        // 1,000 x 1,000 x 1,000 nodes of 264 bytes pass 16 GiB within x = 65
        let m = alloc_failure(1000, 1000, 1000).unwrap();
        assert!(m.starts_with("GCAMalloc: could not allocate 65,"), "{m}");
        assert_eq!(alloc_failure(128, 128, 128), None);
    }

    #[test]
    fn names_are_freesurfer_s() {
        assert_eq!(cma_label_to_name(0), "Unknown");
        assert_eq!(cma_label_to_name(2), "Left_Cerebral_White_Matter");
        assert_eq!(cma_label_to_name(-77), "");
    }
}
